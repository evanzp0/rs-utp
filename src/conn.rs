use std::{io, net::SocketAddr};

use crate::{
    packet::{Packet, PacketBuilder, PacketType},
    time::now_micro,
};
use rand::random;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Action {
    Send(Packet),
    ConnectSuccess(oneshot::Sender<io::Result<()>>),
}

/// Connection 状态
///
/// 注意：
/// 1. 主动方发出 SYN 后，将状态设为 Connecting。
/// 2. 主动方收到 SYN-ACK 后，将状态设为 Connected { is_confirm: true }。
/// 3. 被动方收到 SYN-ACK 后，将状态设为 Connected { is_confirm: faslse }。
///    - 当被动方收到第一个 ST-DATA 后，将状态设为 Connected { is_confirm: true }。
pub enum State {
    Connecting {
        seq_nr: u16,
        waiter: Option<oneshot::Sender<io::Result<()>>>,
    },
    Connected {
        is_confirm: bool,
    },
    Closed,
}

impl Default for State {
    fn default() -> Self {
        State::Closed
    }
}

pub struct Connection {
    state: State,
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
        peer_addr: std::net::SocketAddr,
        waiter: oneshot::Sender<io::Result<()>>,
    ) -> (Self, Packet) {
        let seq_nr = random();

        let mut conn = Self {
            state: State::Connecting {
                seq_nr,
                waiter: Some(waiter),
            },
            conn_id_recv: conn_id,
            conn_id_send: conn_id + 1,
            seq_nr,
            ack_nr: 0,
            peer_addr,
        };

        let syn = conn.syn();
        (conn, syn)
    }

    /// 被动连接，收到 SYN 包后调用，创建 Connection 对象
    pub fn new_passive(syn_pkt: &Packet, addr: std::net::SocketAddr) -> (Self, Packet) {
        let conn = Self {
            state: State::Connected { is_confirm: false },
            conn_id_recv: syn_pkt.conn_id().wrapping_add(1),
            conn_id_send: syn_pkt.conn_id(),
            seq_nr: random(),
            ack_nr: syn_pkt.seq_nr(),
            peer_addr: addr,
        };

        let syn = conn.syn_ack();
        (conn, syn)
    }

    /// 主动连接：生成 SYN 包
    fn syn(&mut self) -> Packet {
        // 规范：ST_SYN 增加 seq_nr + 1，conn_id = self.conn_id_recv

        let syn = PacketBuilder::new(
            PacketType::Syn,
            self.conn_id_recv,
            now_micro(),
            0,
            self.seq_nr,
        )
        .build();

        self.seq_nr += 1;

        syn
    }

    /// 被动连接：生成 SYN-ACK (ST_STATE) 包
    fn syn_ack(&self) -> Packet {
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
    pub fn on_packet(&mut self, pkt: &Packet) -> Option<Action> {

        match &mut self.state {
            State::Connecting { seq_nr, waiter } => {
                if pkt.packet_type() == PacketType::State && pkt.ack_nr() == *seq_nr {
                    self.ack_nr = pkt.seq_nr() - 1;

                    let tx = waiter.take();
                    let action = tx.map(|v| Action::ConnectSuccess(v));

                    self.state = State::Connected { is_confirm: true };

                    action
                } else {
                    None
                }
            }
            State::Connected { is_confirm } => match pkt.packet_type() {
                PacketType::Data => {
                    if !*is_confirm {
                        *is_confirm = true;
                    }

                    self.ack_nr = pkt.seq_nr();

                    let ack = PacketBuilder::new(
                        PacketType::State,
                        self.conn_id_send,
                        now_micro(),
                        0,
                        self.seq_nr,
                    )
                    .ack_nr(self.ack_nr)
                    .build();

                    Some(Action::Send(ack))
                }
                _ => None,
            },
            _ => None,
        }
    }
}
