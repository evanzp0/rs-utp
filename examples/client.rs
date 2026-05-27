use std::{io, net::SocketAddr};

use rs_utp::{packet::{PacketBuilder, PacketType}, socket::UtpSocket, time::now_micro};

#[tokio::main]
async fn main() -> io::Result<()> {
    let local_addr: SocketAddr = "0.0.0.0:19001".parse().unwrap(); // 随机端口
    let server_addr: SocketAddr = "127.0.0.1:19000".parse().unwrap();

    let socket = UtpSocket::bind(local_addr).await?;
    
    println!("Connecting to {}...", server_addr);
    match socket.connect(server_addr).await {
        Ok(stream) => {
            println!("Connect success! Peer: {}", stream.peer_addr());
            
            // V0.2 关键验证：客户端连接成功后，必须发送一个 ST_DATA 包！
            // 否则服务端的 Connection 永远不会从 Connecting 变为 Connected (AcceptReady)
            println!("Simulating sending first DATA packet to complete server handshake...");
            
            // 发送 DATA 包的方法，这里仅作伪代码演示
            socket.send_mock_data(stream.conn_id()).await;
            
            // 真实场景中，V0.3 实现了 stream.write() 后，这里会是：
            // stream.write(b"hello").await?;
        }
        Err(e) => {
            eprintln!("Connect failed: {}", e);
        }
    }

    // 阻塞主线程，防止退出
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    Ok(())
}