//! SPSC 通道封装 — crossbeam bounded channel。
//!
//! 数据流：FFI callback → Sender → bounded(4096) → Receiver → tokio task → Pipeline

use crossbeam::channel::{self, Receiver, Sender};
use taiji_engine::types::tick::TickData;

/// SPSC 通道 — 持有发送端和接收端。
/// 发送端可 clone 多投；接收端不可 clone，通过 `take_receiver` 转移所有权。
pub struct TickChannel {
    tx: Sender<TickData>,
    rx: Receiver<TickData>,
}

impl TickChannel {
    /// 创建容量为 `cap` 的有界通道。
    pub fn with_capacity(cap: usize) -> Self {
        let (tx, rx) = channel::bounded(cap);
        Self { tx, rx }
    }

    /// 获取可 clone 的发送端。
    pub fn sender(&self) -> Sender<TickData> {
        self.tx.clone()
    }

    /// 取出接收端（消耗 TickChannel）。
    pub fn take_receiver(self) -> Receiver<TickData> {
        self.rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn spsc_send_recv() {
        let ch = TickChannel::with_capacity(4);
        let tx = ch.sender();
        let rx = ch.take_receiver();

        let tick = TickData::default();
        tx.send(tick.clone()).unwrap();

        let received = rx.recv().unwrap();
        assert_eq!(received.instrument, tick.instrument);
    }

    #[test]
    fn spsc_multiple_sends() {
        let ch = TickChannel::with_capacity(8);
        let tx = ch.sender();
        let rx = ch.take_receiver();

        for i in 0..5 {
            let mut tick = TickData::default();
            tick.instrument = format!("rb{:04}", i);
            tx.send(tick).unwrap();
        }

        let mut count = 0;
        while let Ok(tick) = rx.try_recv() {
            assert!(tick.instrument.starts_with("rb"));
            count += 1;
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn spsc_cross_thread() {
        let ch = TickChannel::with_capacity(4);
        let tx = ch.sender();
        let rx = ch.take_receiver();

        let handle = thread::spawn(move || {
            let mut tick = TickData::default();
            tick.last_price = 4200.0;
            tx.send(tick).unwrap();
        });

        let received = rx.recv().unwrap();
        assert_eq!(received.last_price, 4200.0);
        handle.join().unwrap();
    }
}
