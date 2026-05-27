use std::{io, net::SocketAddr};

use crate::{
    packet::{Packet, PacketBuilder, PacketType},
    time::now_micro,
};
use rand::random;
use tokio::sync::oneshot;

pub enum Action {
    Send(Packet),
    ConnectSuccess(oneshot::Sender<io::Result<()>>),
    AcceptReady {
        recv_id: u16,
        peer_addr: std::net::SocketAddr,
    },
}

pub enum UtpState {
    Connecting(Option<oneshot::Sender<io::Result<()>>>),
    Connected,
    Closed,
}

impl Default for UtpState {
    fn default() -> Self {
        UtpState::Closed
    }
}

pub struct Connection {
    state: UtpState,
    conn_id_recv: u16,
    conn_id_send: u16,
    seq_nr: u16,
    ack_nr: u16,
    peer_addr: SocketAddr,
}

impl Connection {
    pub fn conn_id_recv(&self) -> u16 {
        self.conn_id_recv
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    /// 主动连接时，创建 Connection 对象
    pub fn new_active(
        conn_id: u16,
        addr: std::net::SocketAddr,
        waiter: oneshot::Sender<io::Result<()>>,
    ) -> Self {
        Self {
            state: UtpState::Connecting(Some(waiter)),
            conn_id_recv: conn_id,
            conn_id_send: conn_id + 1,
            seq_nr: random(),
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
            seq_nr: random(),
            ack_nr: syn_pkt.seq_nr(),
            peer_addr: addr,
        }
    }

    /// 主动连接：生成 SYN 包
    pub fn syn(&mut self) -> Packet {
        // 规范：ST_SYN 增加 seq_nr + 1，conn_id = self.conn_id_recv

        let syn = PacketBuilder::new(PacketType::Syn, self.conn_id_recv, now_micro(), 0, random())
            .build();

        self.seq_nr += 1;

        syn
    }

    /// 被动连接：生成 SYN-ACK (ST_STATE) 包
    pub fn syn_ack(&mut self) -> Packet {
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

    pub fn mock_data(&mut self) -> Packet {
        let data = PacketBuilder::new(
            PacketType::Data,
            self.conn_id_send,
            now_micro(),
            0,
            self.seq_nr,
        )
        .ack_nr(self.ack_nr)
        .build();

        self.seq_nr += 1;

        data
    }

    /// 被动反应：处理收到的 packet，驱动状态机s
    pub fn on_packet(&mut self, pkt: &Packet) -> Vec<Action> {
        match &mut self.state {
            UtpState::Connecting(waiter) => {
                match pkt.packet_type() {
                    // 收到 SYN-ACK (ST_STATE)
                    PacketType::State => {
                        self.ack_nr = pkt.seq_nr() - 1;

                        let tx = waiter.take();
                        let mut actions = vec![];
                        if let Some(tx) = tx {
                            actions.push(Action::ConnectSuccess(tx));
                        }

                        self.state = UtpState::Connected;
                        return actions;
                    }

                    // 收到第一个 ST-DATA 包
                    PacketType::Data => {
                        self.ack_nr = pkt.seq_nr();
                        self.state = UtpState::Connected;

                        let ack = PacketBuilder::new(
                            PacketType::State,
                            self.conn_id_send,
                            now_micro(),
                            0,
                            self.seq_nr,
                        )
                        .ack_nr(self.ack_nr)
                        .build();

                        vec![
                            Action::Send(ack),
                            Action::AcceptReady {
                                recv_id: self.conn_id_recv,
                                peer_addr: self.peer_addr,
                            },
                        ]
                    }

                    _ => vec![],
                }
            }
            _ => vec![],
        }
    }
}
