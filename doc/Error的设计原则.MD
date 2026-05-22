# Error 的设计原则

在 Rust 中设计库的错误处理时，**强烈建议采用按领域/模块分类，使用细粒度的 enum，然后再用一个总的 enum 包装它们** ;

而不是一个 **大型扁平的 Enum Error** 包含了所有 Error，它在小型脚本或简单应用中可能没问题，但在一个网络协议库中，它是一个**反模式**。

以下是为什么第一种方式更好，以及如何在 Rust 中优雅地实现它。

### 为什么不建议用大型扁平 Enum ？

1. **违背了 Rust 的穷尽匹配特性**
   如果你的 `parse_packet` 函数返回了 `UtpError`，调用者使用 `match` 时，Rust 会**强制**它处理 `TimedOut` 或 `ConnectionReset` 等变体。
   但解析数据包的阶段根本不可能发生连接超时！这迫使调用者写无用的 `_ => {}` 或处理不可能发生的逻辑，失去了 Rust 类型系统保护你的意义。
2. **命名冲突与语义模糊**
   比如， `EmptyDataPayload` 这样的错误，在 Packet 层**读取到一个没有 payload 的包**，和 Connection 层**期望收到数据却收到了空包**都可能发生，但语义不同。
   扁平 Enum 会导致命名困难（比如 `EmptyDataPayloadPacket` vs `EmptyDataPayloadConnection`），非常丑陋。
3. **信息丢失**
   例如，在**大型扁平的 Enum Error**中，可能会使用类似 `InvalidPacket(String)` 的变体，其中使用了字符串描述更底层的错误类型。
   这导致失去了结构化信息，调用者无法用 `match` 精准提取错误详情，只能做字符串匹配，这是极其脆弱的。
4. **库的演进**
   当你新增一种错误时，扁平 Enum 会破坏所有使用该库的 `match` 语句。而分类 Enum 只会破坏处理该特定分类的代码。

---

### 推荐的最佳实践：层级式错误设计

结合 Rust 生态中最常用的 [`thiserror`](https://crates.io/crates/thiserror) 库，我们可以优雅地实现第一种方式。
核心思想是：**底层用细粒度 Enum 描述具体错误，顶层用 Enum 统一对外。**

#### 1. 底层：按领域划分的细粒度错误

```rust
use thiserror::Error;

// 数据包解析领域的错误
#[derive(Debug, Error)]
pub enum PacketError {
    #[error("Invalid header: {0}")]
    InvalidHeader(#[from] PacketHeaderError),
    
    #[error("Invalid extension: {0}")]
    InvalidExtension(#[from] ExtensionError),
    
    #[error("Invalid packet length")]
    InvalidLen,
    
    #[error("Empty data payload")]
    EmptyDataPayload,
}

#[derive(Debug, Error)]
pub enum PacketHeaderError {
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    // ...
}

// 连接状态机领域的错误
#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("Connection reset by peer")]
    Reset,
    
    #[error("Connection timed out")]
    TimedOut,
    
    #[error("Invalid FIN received")]
    InvalidFin,
    
    #[error("Invalid SYN received")]
    InvalidSyn,
    
    #[error("SYN from acceptor")]
    SynFromAcceptor,
    
    #[error("Empty data payload on connection")]
    EmptyDataPayload,
}
```

#### 2. 顶层：统一的错误包装器

当你的库对外提供公共 API（比如 `UtpSocket::connect` 或 `UtpSocket::recv`）时，你需要一个总的错误类型返回给用户。

```rust
#[derive(Debug, Error)]
pub enum UtpError {
    #[error("Packet error: {0}")]
    Packet(#[from] PacketError),
    
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Buffer too small")]
    BufferTooSmall,
    
    #[error("Would block")]
    WouldBlock,
}
```

#### 3. 内部函数签名保持精准

这就是这种设计最强大的地方：**内部函数只返回它们可能产生的错误，而不是总错误。**

```rust
// 内部解析函数：只返回 PacketError
fn parse_packet(buf: &[u8]) -> Result<Packet, PacketError> {
    if buf.is_empty() {
        return Err(PacketError::EmptyDataPayload);
    }
    // ...
    Ok(Packet {})
}

// 内部连接函数：只返回 ConnectionError
fn handle_syn(conn: &mut Connection) -> Result<(), ConnectionError> {
    // ...
    Err(ConnectionError::SynFromAcceptor)
}

// 对公开的 API：返回 UtpError，利用 `?` 自动转换
pub fn process_event(socket: &mut UtpSocket) -> Result<(), UtpError> {
    let packet = parse_packet(&socket.buf)?; // PacketError 自动通过 #[from] 转为 UtpError::Packet
    handle_syn(&mut socket.conn)?;           // ConnectionError 自动转为 UtpError::Connection
    Ok(())
}
```

### 调用者如何使用？（体验极佳）

调用你的库的人，可以根据自己的需求选择处理粒度：

```rust
match socket.process_event() {
    Ok(_) => println!("Success"),
    Err(UtpError::Connection(ConnectionError::TimedOut)) => {
        // 精准处理超时
        println!("Timeout, retrying...");
    }
    Err(UtpError::Packet(PacketError::EmptyDataPayload)) => {
        // 精准处理空包
        println!("Empty packet ignored");
    }
    Err(UtpError::Io(e)) => {
        // 处理底层 IO 错误
        eprintln!("Fatal IO: {}", e);
    }
    Err(e) => {
        // 兜底处理其他所有不关心的错误
        eprintln!("Other UTP error: {}", e);
    }
}
```

### 总结

1. **不要用 String 传递错误**：之前例子中提到的 `InvalidPacket(String)` 是反模式，无法 `match`，用 `thiserror` 的结构化代替。
2. **不要用扁平大 Enum**：它会污染函数签名，让 `match` 变得痛苦，强行把不相关的错误拉到同一个抽象层。
3. **采用层级设计**：底层 `PacketError` / `ConnectionError` 保证函数签名的精准性和类型安全；顶层 `UtpError` 配合 `#[from]` 和 `?` 操作符，对用户提供统一的错误边界。这正是 Rust 标准库（如 `std::io`）和优秀开源库（如 `reqwest`, `rusqlite`）采用的模式。