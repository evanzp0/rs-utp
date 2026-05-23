use std::io;

use bytes::{BufMut, BytesMut};
use rs_utp::packet::{PACKET_HEADER_LEN, Packet};
use tokio::net::UdpSocket;

pub async fn send_packet(
    socket: &UdpSocket,
    addr: &std::net::SocketAddr,
    packet: &Packet,
    payload: &[u8],
) -> io::Result<usize> {
    let mut buf = BytesMut::with_capacity(PACKET_HEADER_LEN + payload.len());
    packet.encode_to(&mut buf);
    buf.put_slice(payload);

    socket.send_to(&buf, addr).await
}
