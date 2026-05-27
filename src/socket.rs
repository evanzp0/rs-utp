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

pub struct UtpListener {
    rx: mpsc::Receiver<UtpStream>,
}
impl UtpListener {
    pub async fn accept(&mut self) -> io::Result<UtpStream> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Closed"))
    }
}

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

    pub fn listen(&self) -> UtpListener {
        let (tx, rx) = mpsc::channel(1024);
        let mut inner = self.inner.lock().expect("get inner failed");
        inner.accept_tx = Some(tx);

        UtpListener { rx }
    }

    pub async fn connect(&self, addr: std::net::SocketAddr) -> io::Result<UtpStream> {
        let (tx, rx) = oneshot::channel();

        let conn_id = random();
        let mut conn = Connection::new_active(conn_id, addr, tx);
        let syn_pkt = conn.syn();
        let _ = self.udp.send_to(syn_pkt.encode().as_ref(), addr).await;

        let mut inner = self.inner.lock().expect("get inner failed");
        inner.conns.insert(conn_id, conn);
        drop(inner);

        let _ = rx
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::ConnectionRefused, "Closed"))?;

        Ok(UtpStream::new(conn_id, addr))
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
        let (actions, accept_tx) = {
            let mut inner = inner.lock().unwrap();

            let actions = if pkt.packet_type() == PacketType::Syn {
                let recv_id = pkt.conn_id().wrapping_add(1);
                if inner.conns.contains_key(&recv_id) {
                    vec![]
                } else {
                    let mut conn = Connection::new_passive(&pkt, addr);
                    let syn_ack_pkt = conn.syn_ack();
                    inner.conns.insert(conn.conn_id_recv(), conn);
                    vec![Action::Send(syn_ack_pkt)]
                }
            } else {
                let recv_id = pkt.conn_id();
                if let Some(conn) = inner.conns.get_mut(&recv_id) {
                    conn.on_packet(&pkt)
                } else {
                    vec![]
                }
            };

            let tx_clone = inner.accept_tx.clone();
            (actions, tx_clone)
        };

        println!("recv {:?}", pkt.packet_type());

        for action in actions {
            match action {
                Action::Send(pkt_to_send) => {
                    udp.send_to(pkt_to_send.encode().as_ref(), addr).await.ok();
                }
                Action::ConnectSuccess(tx) => {
                    tx.send(Ok(())).ok();
                }
                Action::AcceptReady { recv_id, peer_addr } => {
                    if let Some(tx) = accept_tx.as_ref() {
                        let stream = UtpStream::new(recv_id, peer_addr);
                        tx.try_send(stream).ok();
                    }
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

            self.udp
                .send_to(data_packet.encode().as_ref(), conn.peer_addr())
                .await
                .ok();
        }

        ()
    }
}
