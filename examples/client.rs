mod utils;

use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::BytesMut;
use rs_utp::packet::{Packet, PacketBuilder, PacketType};
use tokio::net::UdpSocket;

use crate::utils::send_packet;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let server_addr = "127.0.0.1:12345".parse().unwrap();
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u32;
    
    // 发送一个请求包
    let request = PacketBuilder::new(
        PacketType::Data,
        100,
        timestamp,
        1024 * 1024,
        1,
    )
    .ack_nr(0)
    .build();
    let payload = b"hello utp!";
    
    let sent = send_packet(&socket, &server_addr, &request, payload).await?;
    println!("Sent len: {} ", sent);

    // 等待响应（只处理第一个包）
    let mut raw_buf = [0u8; 65535];
    let (len, _) = socket.recv_from(&mut raw_buf).await?;

    let mut buf = BytesMut::from(&raw_buf[..len]);
    let result = Packet::decode(&mut buf);

    match result {
        Ok(packet) => {
            println!(
                "Received ACK: conn_id={}, ack_nr={}, type={}", 
                packet.conn_id(),
                packet.ack_nr(),
                packet.packet_type(),
            );
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}