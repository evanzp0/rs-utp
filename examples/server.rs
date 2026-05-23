mod utils;

use std::io;

use bytes::BytesMut;
use rs_utp::packet::{Packet, PacketBuilder, PacketType};
use tokio::net::UdpSocket;

use crate::utils::send_packet;

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:12345").await?;
    let mut raw_buf = [0u8; 65535];

    loop {
        let (len, addr) = socket.recv_from(&mut raw_buf).await?;
        println!("udp recv from {:?} , len: {} ", addr, len);

        let mut buf = BytesMut::from(&raw_buf[0..len]);

        match Packet::decode(&mut buf) {
            Ok(packet) => {
                println!(
                    "Received uTP packet from {}: seq={}, ack={}",
                    addr,
                    packet.seq_nr(),
                    packet.ack_nr(),
                );

                 // 发送响应示例
                let packet_builder = PacketBuilder::new(
                        PacketType::State, 
                        packet.conn_id(), 
                        0, 
                        1024 * 1024, 
                        0,
                    ).ack_nr(packet.seq_nr());

                let packet: Packet = packet_builder.build();

                let sent = send_packet(&socket, &addr,&packet).await?;
                println!("Sent len: {} ", sent);
            }
            Err(e) => {
                eprintln!("Error receiving packet: {}", e);
            }
        }


    }
}