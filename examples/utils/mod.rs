use std::io;

use bytes::BytesMut;
use rs_utp::packet::{PACKET_HEADER_LEN, Packet};
use tokio::net::UdpSocket;

// 调用方这样写，一次分配，整包写入，零拷贝发送
pub async fn send_packet(socket: &UdpSocket, addr: &std::net::SocketAddr, packet: &Packet) -> io::Result<usize> {
    let mut buf = BytesMut::with_capacity(PACKET_HEADER_LEN);
    packet.encode_to(&mut buf);
    
    socket.send_to(&buf, addr).await
}