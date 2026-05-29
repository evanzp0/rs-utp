use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};

use rand::random;
use tokio::{
    net::UdpSocket,
    sync::{mpsc, oneshot},
};

use crate::{
    conn::{Action, Connection},
    packet::{Packet, PacketType},
    stream::UtpStream,
};

struct SocketInner {
    conns: HashMap<u16, Connection>,
    accept_tx: Option<mpsc::Sender<UtpStream>>,
}

pub struct UtpSocket {
    udp: Arc<UdpSocket>,
    inner: Arc<Mutex<SocketInner>>,
}

impl UtpSocket {
    pub async fn bind(addr: std::net::SocketAddr) -> io::Result<Self> {
        let udp = UdpSocket::bind(addr).await?;
        let udp = Arc::new(udp);

        let inner = Arc::new(Mutex::new(SocketInner {
            conns: HashMap::new(),
            accept_tx: None,
        }));
        let socket = Self { udp, inner };

        tokio::spawn(Self::run(socket.udp.clone(), socket.inner.clone()));

        Ok(socket)
    }

    pub async fn accept(&self) -> io::Result<UtpStream> {
        let (tx, mut rx) = mpsc::channel(1);

        {
            let mut inner = self.inner.lock().expect("get inner failed");
            inner.accept_tx = Some(tx);
        }

        rx.recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Closed"))
    }

    pub async fn connect(&self, peer_addr: std::net::SocketAddr) -> io::Result<UtpStream> {
        let (tx, rx) = oneshot::channel();

        let conn_id = random();
        let (conn, syn) = Connection::new_active(conn_id, peer_addr, tx);

        let _rst = self.udp.send_to(syn.encode().as_ref(), peer_addr).await;
        println!("send syn: {:#?}, rst: {:#?}", syn, _rst);

        {
            let mut inner = self.inner.lock().expect("get inner failed");
            inner.conns.insert(conn_id, conn);
        }

        let _ = rx
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::ConnectionRefused, "Closed"))?;

        Ok(UtpStream::new(conn_id, peer_addr))
    }

    async fn run(udp: Arc<UdpSocket>, inner: Arc<Mutex<SocketInner>>) {
        let mut buf = [0u8; 1500];
        loop {
            if let Ok((len, addr)) = udp.recv_from(&mut buf).await {
                let mut data = &buf[..len];
                if let Ok(pkt) = Packet::decode(&mut data) {
                    // 传递 &mut &[u8]
                    Self::handle_packet(&udp, &inner, pkt, addr).await;
                }
            }
        }
    }

    async fn handle_packet(
        udp: &Arc<UdpSocket>,
        inner: &Arc<Mutex<SocketInner>>,
        pkt: Packet,
        addr: std::net::SocketAddr,
    ) {
        println!("on packet: {:#?}", pkt);

        let mut action = None;

        if pkt.packet_type() == PacketType::Syn {
            let mut inner = inner.lock().expect("get socket inner error");

            // 收到 SYN 包，建立 Connection，发送 SYN-ACK
            let send_id = pkt.conn_id();
            let recv_id = send_id.wrapping_add(1);

            if !inner.conns.contains_key(&recv_id) {
                let (conn, syn_ack) = Connection::new_passive(&pkt, addr);
                inner.conns.insert(conn.conn_id_recv(), conn);

                if let Some(tx) = &inner.accept_tx {
                    // 通知被动连接 accept()
                    let stream = UtpStream::new(recv_id, addr);
                    let _ = tx.try_send(stream);
                }
                
                action = Some(Action::Send(syn_ack))
            }
        } else {
            // 非 SYN 包，都转交 Connection 处理
            let mut inner = inner.lock().expect("get socket inner error");

            let recv_id = pkt.conn_id();
            if let Some(conn) = inner.conns.get_mut(&recv_id) {
                action = conn.on_packet(&pkt);
            }
        }

        if let Some(v) = action {
            match v {
                Action::Send(pkt_to_send) => {
                    let _rst = udp.send_to(pkt_to_send.encode().as_ref(), addr).await;
                    println!("auto send: {:#?}, rst: {:#?}", pkt_to_send, _rst);
                }
                Action::ConnectSuccess(tx) => {
                    // 通知主动连接 connection()
                    tx.send(Ok(())).ok();
                }
            }
        }
    }
}

impl UtpSocket {
    pub async fn send_mock_data(&self, conn_id: u16) {
        let mut inner = self.inner.lock().unwrap();
        
        if let Some(conn) = inner.conns.get_mut(&conn_id) {
            let data_packet = conn.mock_data();

            println!("send mock data: {:#?}", data_packet);

            self.udp
                .send_to(data_packet.encode().as_ref(), conn.peer_addr())
                .await
                .ok();
        }
    }
}
