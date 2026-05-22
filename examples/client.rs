use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use rs_utp::packet::{PacketBuilder, PacketCodec, PacketType};
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let server_addr = "127.0.0.1:12345".parse().unwrap();
    
    let mut framed = UdpFramed::new(socket, PacketCodec);
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u32;
    
    // 发送一个请求包
    let request = PacketBuilder::new(
        PacketType::Data,
        0x1234,
        timestamp,
        1024 * 1024,
        1,
    )
    .seq_nr(1)
    .build();
    
    println!("Sending packet to {}", server_addr);
    framed.send((&request, server_addr)).await?;
    
    // 等待响应（只处理第一个包）
    if let Some(result) = framed.next().await {
        match result {
            Ok((packet, addr)) => {
                println!(
                    "Received response from {}: seq={}, ack={}",
                    addr,
                    packet.seq_nr(),
                    packet.ack_nr(),
                );
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
    
    Ok(())
}