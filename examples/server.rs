use std::io;

use futures::{SinkExt, StreamExt};
use rs_utp::packet::{PacketBuilder, PacketCodec, PacketType};
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:12345").await?;
    
    // 使用 UdpFramed 包装 socket，传入我们的编解码器
    let mut framed = UdpFramed::new(socket, PacketCodec);

    // 此时 framed 实现了 Stream<Item = Result<(Packet, SocketAddr), io::Error>>
    while let Some(result) = framed.next().await {
        match result {
            Ok((packet, addr)) => {
                println!(
                    "Received uTP packet from {}: seq={}, ack={}",
                    addr,
                    packet.seq_nr(),
                    packet.ack_nr(),
                );
                
                // 你的 uTP 逻辑：处理包、发送 ACK 等...
                
                // 发送响应示例
                let packet_builder = PacketBuilder::new(
                        PacketType::State, 
                        packet.conn_id(), 
                        0, 
                        1024 * 1024, 
                        0,
                    ).ack_nr(packet.seq_nr());

                let response = packet_builder.build();

                // 直接发送结构体，Encoder 会自动序列化
                if let Err(e) = framed.send((&response, addr)).await {
                    eprintln!("Error sending response to {}: {}", addr, e);
                }
            }
            Err(e) => {
                eprintln!("Error receiving packet: {}", e);
            }
        }
    }

    Ok(())
}