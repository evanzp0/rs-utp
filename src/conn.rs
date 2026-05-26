use std::{io, net::SocketAddr};

use crate::{
    packet::{Packet, PacketBuilder, PacketType},
    time::now_micro,
    utils::random_nr,
};
use tokio::sync::oneshot;

pub enum UtpAction {
    Send(Packet),
    ConnectSuccess(oneshot::Sender<io::Result<()>>),
    AcceptReady,
}

pub enum UtpState {
    Connecting(Option<oneshot::Sender<io::Result<()>>>),
    Connected,
    Closed,
}

pub struct UtpConnection {
    state: UtpState,
    conn_id_recv: u16,
    conn_id_send: u16,
    seq_nr: u16,
    ack_nr: u16,
    peer_addr: SocketAddr,
}

impl UtpConnection {
    /// 主动连接时，创建 Connection 对象
    pub fn new_active(addr: std::net::SocketAddr, waiter: oneshot::Sender<io::Result<()>>) -> Self {
        let recv_id = random_nr();
        Self {
            state: UtpState::Connecting(Some(waiter)),
            conn_id_recv: recv_id,
            conn_id_send: recv_id + 1,
            seq_nr: random_nr(),
            ack_nr: 0,
            peer_addr: addr,
        }
    }

    /// 被动连接，收到 SYN 包后调用，创建 Connection 对象
    pub fn new_passive(syn_pkt: &Packet, addr: std::net::SocketAddr) -> Self {
        Self {
            state: UtpState::Connecting(None),
            conn_id_recv: syn_pkt.conn_id().wrapping_add(1),
            conn_id_send: syn_pkt.conn_id(),
            seq_nr: random_nr(),
            ack_nr: syn_pkt.seq_nr(),
            peer_addr: addr,
        }
    }

    /// 主动连接：生成 SYN 包
    pub fn connect(&mut self) -> Packet {
        // 规范：ST_SYN 增加 seq_nr + 1，conn_id = self.conn_id_recv

        let syn = PacketBuilder::new(
            PacketType::Syn,
            self.conn_id_recv,
            now_micro(),
            0,
            random_nr(),
        )
        .build();

        self.seq_nr += 1;

        syn
    }

    /// 被动连接：生成 SYN-ACK (ST_STATE) 包
    pub fn accept(&mut self) -> Packet {
        // 规范：ST_STATE 不增加 seq_nr，conn_id = self.conn_id_send

        let syn_ack = PacketBuilder::new(
            PacketType::State,
            self.conn_id_send,
            now_micro(),
            0,
            self.seq_nr,
        )
        .ack_nr(self.ack_nr)
        .build();

        syn_ack
    }

}
