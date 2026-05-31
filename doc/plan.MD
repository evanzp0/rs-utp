# rust 开发 uTP 库计划里程碑（Actor 增量迭代·优化版）

## 总体原则
- **每阶段都能编译运行**，产出可演示的功能
- **Actor 模型贯穿始终**：`UtpSocket`（路由器） + `UtpConnection`（每连接状态机） + `UtpStream`（用户接口）
- **先通数据流，再管握手，最后保可靠**

---

## V0.1：跑通 Header 与极简 UDP 收发

**阶段目标**：验证 Rust 位运算与网络字节序，打通最底层的物理通道。无异步、无 Actor。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **UtpHeader** | 定义结构体；实现 `try_from_bytes(&[u8])` 和 `to_bytes() -> [u8;20]`；处理 `type_ver` 位运算（高4位Type，低4位Ver）；处理扩展字段标志位 | ⭐⭐⭐⭐⭐ |
| **UDP 收发** | 使用 `std::net::UdpSocket` 同步阻塞 `send_to` / `recv_from`，硬编码目标地址 | ⭐ |
| **测试标准** | 构造 DATA 类型 Header，序列化→UDP 发出→另一端收到→反序列化→字段一致 | |

**产出**：一个可运行的二进制，能发送/接收并解析 uTP Header。

---

## V0.2：建立 Actor 骨架与单向数据流

**阶段目标**：强行假设连接已建立（无握手），让数据能**单向**从用户流向网络。建立三个 Actor 的通信骨架。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **ActorRef\<M\>** | 封装 `UnboundedSender`（V0.7 再改有界），提供 `tell()` 方法 | ⭐⭐⭐⭐⭐ |
| **UtpStream** | 提供 `async write(buf)`，内部发送 `Stream2ConnMsg::SendData` | ⭐⭐ |
| **UtpConnection** | 硬编码状态为 Connected；收到 `SendData` 后切片（暂不按 MTU，固定 1400 字节），构造 Header（seq_nr 递增），发送 `Conn2SocketMsg::SendPacket` | ⭐⭐⭐ |
| **UtpSocket** | 绑定 `tokio::net::UdpSocket`；接收 `SendPacket` 并调用 `send_to` | ⭐⭐ |
| **测试标准** | 客户端调用 `stream.write(b"hello")`，Wireshark 抓包能看到 UDP 负载中包含 uTP Header + "hello" | |

**产出**：能单向发包的 demo，用户调用 write 就能看到网络上有包发出。

---

## V0.3：双向数据流与基础 ACK

**阶段目标**：跑通 Echo 回显。打通网络→Socket→Connection→Stream 的完整回路。实现“收到 DATA 就回 ACK”的傻瓜逻辑。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **UtpSocket** | 启动后台 Task 循环 `recv_from`；解析 Header，提取 ConnID，从路由表查找 `ActorRef<ConnMsg>`，转发 `IncomingPacket` | ⭐⭐⭐⭐⭐ |
| **UtpConnection** | 处理 `IncomingPacket`：若是 DATA 包，payload 存入简易接收缓冲区（`VecDeque<u8>`），立即构造 ACK 包发回 | ⭐⭐⭐⭐ |
| **UtpStream** | 提供 `async read(buf)`，从接收缓冲区读数据返回 | ⭐⭐ |
| **简易缓冲区** | 发送和接收各一个 `VecDeque<u8>`，只处理顺序，不做窗口限制，发完就扔 | ⭐ |
| **测试标准** | Echo 服务端 + 客户端：客户端发数据，服务端原样回显，客户端能打印回显内容（局域网不丢包） | |

**产出**：一个能跑 Echo 的不可靠 uTP 库！虽然不处理丢包，但双向通路完全打通。

---

## V0.4：连接生命周期管理（握手与关闭）

**阶段目标**：在现有数据通道上，加上“连接怎么来、怎么没”的逻辑。实现三次握手与四次挥手。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **UtpConnection** | 引入 `ConnState` 枚举（`SynSent`, `SynReceived`, `Connected`, `FinWait1`, `FinWait2`, `Closed`）；根据 `ConnectionOrigin`（主动/被动）处理 SYN/FIN 状态流转 | ⭐⭐⭐⭐⭐ |
| **UtpSocket** | 实现路由表动态增删：收到 SYN 且路由表无对应 Conn 时，`spawn` 新 `UtpConnection` 并注册；接收 `DestroyConnection` 消息移除路由 | ⭐⭐⭐⭐ |
| **UtpStream** | 封装 `connect()`（等待状态变 Connected）和 `close()`（发送 Close 消息） | ⭐⭐⭐ |
| **测试标准** | 客户端能通过三次握手 `connect()` 成功，服务端能 `accept()`；双方能正常断开，Actor 资源正确释放（无内存泄漏） | |

**产出**：完整的连接式 uTP，能握手、能通信、能关闭。

---

## V0.5：超时重传 + RTT 估算（固定窗口）

**阶段目标**：先解决“丢包了能重传”，窗口固定为 64KB，不做复杂流控。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **UtpConnection** | 引入 `tokio::select!`，同时监听通道消息和定时器；处理 `Timeout` 消息，从 SendBuffer 取出旧包重传 | ⭐⭐⭐⭐⭐ |
| **SendBuffer** | 维护已发未确认队列：`VecDeque<(seq_nr, payload, time_sent)>`；收到 ACK 后剔除；提供 `iter_pending()` 供重传使用 | ⭐⭐⭐⭐ |
| **RTT 估算器** | 实现 SRTT、RTTVar、RTO 计算（标准 TCP 算法），每个 ACK 更新 | ⭐⭐⭐⭐ |
| **测试标准** | Socket 层注入 10% 随机丢包，发送小文件（<64KB），能完整接收，重传生效 | |

**产出**：可靠传输的基础能力，丢包场景下能自动重传。

---

## V0.6：真正的滑动窗口与乱序重组

**阶段目标**：突破固定窗口限制，支持大文件传输，处理乱序包。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **SendBuffer（重写）** | 按 MTU（1400 字节）切片用户数据；维护 `in_flight` 字节数；实现 `window_available()` 判断是否可发新包；支持 `poll_transmit()` 按窗口提取待发包 | ⭐⭐⭐⭐⭐ |
| **RecvBuffer（重写）** | 使用 `BTreeMap<seq_nr, Payload>` 暂存乱序包；检查连续性，将连续数据推入读缓冲区；计算剩余容量，生成通告窗口 `wnd_size` 写入 ACK | ⭐⭐⭐⭐⭐ |
| **UtpStream** | 实现真正的 `AsyncRead` / `AsyncWrite` trait；缓冲区空/满时正确保存 `Waker` 并返回 `Poll::Pending` | ⭐⭐⭐ |
| **背压（简化版）** | RecvBuffer 满时发送 `wnd_size=0` 的 ACK；SendBuffer 检查对端通告窗口 | ⭐⭐ |
| **测试标准** | 注入 10% 丢包 + 乱序，发送 10MB 文件，接收端完整无损接收 | |

**产出**：完整的可靠传输 + 流控，支持任意大小文件传输。

---

## V0.7：实现 SACK 扩展

**阶段目标**：优化高丢包环境下的重传效率，避免“滑窗卡死”。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **UtpHeader/Ext** | 解析和序列化 SACK 扩展位图（扩展类型=1） | ⭐⭐⭐ |
| **RecvBuffer** | 根据 `BTreeMap` 中的乱序缺口，精准生成 SACK 位图，附加在 ACK 中发出 | ⭐⭐⭐⭐ |
| **SendBuffer** | 解析 SACK 位图，精准标记哪些包已被对端接收；超时重传时跳过已 SACK 的包，只重传真正丢失的 | ⭐⭐⭐⭐⭐ |
| **测试标准** | 20% 高丢包率下，对比开启 SACK 前后的传输速率，SACK 应有明显提升 | |

**产出**：高丢包环境下的传输效率优化。

---

## V0.8：实现 LEDBAT 延迟计算与拥塞控制

**阶段目标**：让 uTP 具备“低优先级”特征，在带宽竞争时主动退让。

| 组件 | 实现内容 | 侧重 |
|------|----------|------|
| **UtpHeader** | 激活 `timestamp_microseconds` 和 `timestamp_difference_microseconds` 字段 | ⭐⭐ |
| **UtpConnection** | 新增 LEDBAT 上下文：维护 `base_delay`（历史最小延迟队列，窗口大小建议 60 秒采样）、`current_delay`；计算延迟梯度，动态调整 `cwnd`；实现慢启动 ↔ 拥塞避免状态流转 | ⭐⭐⭐⭐⭐ |
| **背压（完整版）** | 计算新 `cwnd` 后，若窗口有富余，发送 `Conn2StreamMsg::WindowUpdate`，唤醒被挂起的 `UtpStream::write()` 任务；同时将 ActorRef 从 `UnboundedSender` 改为有界通道，实现真正的背压 | ⭐⭐⭐⭐ |
| **测试标准** | 同时跑 uTP 传输 + TCP iperf 测速：TCP 启动时 uTP 吞吐量明显下降（主动退让）；TCP 结束后 uTP 逐渐回抢带宽 | |

**产出**：完整的、符合 uTP 规范的拥塞控制，具备低优先级特征。

---

## 时间估算（假设 1 人全职）

| 里程碑 | 预估时间 | 累积 |
|--------|----------|------|
| V0.1 | 2 天 | 2 天 |
| V0.2 | 3 天 | 5 天 |
| V0.3 | 4 天 | 9 天 |
| V0.4 | 5 天 | 14 天 |
| V0.5 | 5 天 | 19 天 |
| V0.6 | 6 天 | 25 天 |
| V0.7 | 4 天 | 29 天 |
| V0.8 | 6 天 | 35 天 |

**总计约 7 周**（含缓冲约 9 周）

---

## 版本演进一览

| 版本 | 核心主题 | 新增能力 |
|------|----------|----------|
| V0.1 | Header 序列化 | 打包/解包 |
| V0.2 | Actor 骨架 | 单向发包 |
| V0.3 | 双向数据流 | Echo 回显 |
| V0.4 | 连接管理 | 握手 + 挥手 |
| V0.5 | 超时重传 | 基础可靠性（固定窗口） |
| V0.6 | 滑动窗口 | 大文件 + 乱序重组 |
| V0.7 | SACK | 高丢包优化 |
| V0.8 | LEDBAT | 低优先级拥塞控制 |

---

## 关键决策说明

| 决策 | 原因 |
|------|------|
| V0.2 用 `UnboundedSender` | 避免早期引入背压复杂度，V0.8 再改有界 |
| V0.3 用简易缓冲区 | 先验证双向通路正确性，避免早期陷入乱序处理泥潭 |
| V0.4 才引入握手 | 握手逻辑独立于数据流，先保证数据流正确再加握手更容易调试 |
| V0.5 只做超时重传（固定窗口） | 超时重传 + 滑动窗口是最大难点，分开可控 |

这份计划确保了**每个阶段都有可运行的、可演示的产出**，风险均匀分布，适合探索性开发。