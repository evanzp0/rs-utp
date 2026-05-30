use std::collections::{BTreeMap, VecDeque};

use bytes::Bytes;

use crate::utils::wrapping_less_u16;

#[derive(Clone, Debug)]
pub struct RecvBuffer {
    /// 下一个按顺序期望收到的 seq_nr。
    /// 用于计算发出包的 ack_nr = next_expected_seq - 1
    /// 
    /// 主动方：next_expected_seq = 收到的 SYN-ACK.seq_nr
    /// 被动方：next_expected_seq = 收到的 SYN.seq_nr + 1
    next_expected_seq: u16,

    /// 乱序等待的数据
    out_of_order: BTreeMap<u16, Bytes>,

    /// 顺序可读的数据
    buf: VecDeque<Bytes>,
}

impl RecvBuffer {
    pub fn new(initial_seq: u16) -> Self {
        Self {
            next_expected_seq: initial_seq,
            out_of_order: BTreeMap::new(),
            buf: VecDeque::new(),
        }
    }

    pub fn insert(&mut self, seq: u16, payload: Bytes) {
        if payload.is_empty() {
            return;
        }

        if seq == self.next_expected_seq {
            self.buf.push_back(payload);
            self.next_expected_seq = self.next_expected_seq.wrapping_add(1);

            while let Some(payload) = self.out_of_order.remove(&self.next_expected_seq) {
                self.buf.push_back(payload);
                self.next_expected_seq = self.next_expected_seq.wrapping_add(1);
            }
        } else if !wrapping_less_u16(seq, self.next_expected_seq) {
            self.out_of_order.insert(seq, payload);
        }
    }

    pub fn pop_readable(&mut self) -> Option<Bytes> {
        self.buf.pop_front()
    }

    pub fn ack_nr(&self) -> u16 {
        self.next_expected_seq.wrapping_sub(1)
    }
}
