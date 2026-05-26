use std::time::{Duration, Instant};

lazy_static::lazy_static! {
    static ref START: Instant = Instant::now();
}

pub fn now() -> Duration {
    START.elapsed()
}

pub fn now_micro() -> u32 {
    now().as_micros() as u32
}