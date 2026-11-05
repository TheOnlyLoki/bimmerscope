//! Scripted serial link with simulated time for the protocol tests.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::link::{Parity, SerialLink};

#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    Config(u32, u8, Parity),
    Dtr(bool),
    Rts(bool),
    /// Break state and time (µs).
    Break(bool, u64),
    Purge(u64),
    /// Written bytes and time (µs).
    Write(Vec<u8>, u64),
}

/// Expected request and the ECU replies: (delay in ms after the previous
/// reply or the request, bytes).
pub struct Expect {
    pub request: Vec<u8>,
    pub replies: Vec<(u64, Vec<u8>)>,
}

pub struct MockState {
    pub now: u64,
    /// Adapter echo (K-line loop back of the sent bytes).
    pub echo: bool,
    /// Flip bit 0 of the first echoed byte.
    pub corrupt_echo: bool,
    pub dsr: bool,
    /// Received bytes with arrival time (µs).
    pub rx: VecDeque<(u64, u8)>,
    pub expect: VecDeque<Expect>,
    pending: Vec<u8>,
    pub events: Vec<Ev>,
    /// Writes that did not match the expected request.
    pub unexpected: Vec<Vec<u8>>,
}

#[derive(Clone)]
pub struct Mock(pub Arc<Mutex<MockState>>);

impl Mock {
    pub fn new() -> Mock {
        Mock(Arc::new(Mutex::new(MockState {
            now: 1_000_000,
            echo: true,
            corrupt_echo: false,
            dsr: true,
            rx: VecDeque::new(),
            expect: VecDeque::new(),
            pending: Vec::new(),
            events: Vec::new(),
            unexpected: Vec::new(),
        })))
    }

    pub fn st(&self) -> MutexGuard<'_, MockState> {
        self.0.lock().unwrap()
    }

    /// Expects `request` (including checksum), then sends `replies`.
    pub fn expect(&self, request: &[u8], replies: &[(u64, &[u8])]) {
        self.st().expect.push_back(Expect {
            request: request.to_vec(),
            replies: replies.iter().map(|(d, b)| (*d, b.to_vec())).collect(),
        });
    }

    /// Bytes arriving at absolute time `t_us`.
    pub fn rx_at(&self, t_us: u64, bytes: &[u8]) {
        let mut st = self.st();
        for b in bytes {
            st.rx.push_back((t_us, *b));
        }
        st.rx.make_contiguous().sort_by_key(|(t, _)| *t);
    }

    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.st()
            .events
            .iter()
            .filter_map(|e| {
                if let Ev::Write(b, _) = e {
                    Some(b.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn now(&self) -> u64 {
        self.st().now
    }
}

impl MockState {
    fn available(&self) -> usize {
        self.rx.iter().take_while(|(t, _)| *t <= self.now).count()
    }
}

impl SerialLink for Mock {
    fn set_config(&mut self, baud_rate: u32, data_bits: u8, parity: Parity) -> io::Result<()> {
        self.st()
            .events
            .push(Ev::Config(baud_rate, data_bits, parity));
        Ok(())
    }

    fn set_dtr(&mut self, on: bool) -> io::Result<()> {
        self.st().events.push(Ev::Dtr(on));
        Ok(())
    }

    fn set_rts(&mut self, on: bool) -> io::Result<()> {
        self.st().events.push(Ev::Rts(on));
        Ok(())
    }

    fn dsr(&mut self) -> io::Result<bool> {
        Ok(self.st().dsr)
    }

    fn set_break(&mut self, on: bool) -> io::Result<()> {
        let mut st = self.st();
        let now = st.now;
        st.events.push(Ev::Break(on, now));
        Ok(())
    }

    fn purge_input(&mut self) -> io::Result<()> {
        let mut st = self.st();
        let now = st.now;
        st.rx.retain(|(t, _)| *t > now);
        st.events.push(Ev::Purge(now));
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let mut st = self.st();
        let now = st.now;
        st.events.push(Ev::Write(data.to_vec(), now));
        if st.echo {
            let corrupt = st.corrupt_echo;
            for (i, b) in data.iter().enumerate() {
                let b = if corrupt && i == 0 { b ^ 0x01 } else { *b };
                st.rx.push_back((now, b));
            }
            st.corrupt_echo = false;
        }
        st.pending.extend_from_slice(data);
        let Some(front) = st.expect.front() else {
            return Ok(());
        };
        if st.pending == front.request {
            let exp = st.expect.pop_front().unwrap();
            st.pending.clear();
            let mut t = now;
            for (delay, bytes) in exp.replies {
                t += delay * 1000;
                for b in bytes {
                    st.rx.push_back((t, b));
                }
            }
            st.rx.make_contiguous().sort_by_key(|(t, _)| *t);
        } else if st.pending.len() >= front.request.len() || !front.request.starts_with(&st.pending)
        {
            let p = std::mem::take(&mut st.pending);
            st.unexpected.push(p);
        }
        Ok(())
    }

    fn bytes_to_read(&mut self) -> io::Result<usize> {
        Ok(self.st().available())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut st = self.st();
        let n = st.available().min(buf.len());
        for b in buf.iter_mut().take(n) {
            *b = st.rx.pop_front().unwrap().1;
        }
        Ok(n)
    }

    fn wait_rx(&mut self, ms: u64) {
        self.st().now += ms * 1000;
    }

    fn now_us(&self) -> u64 {
        self.st().now
    }

    fn sleep_ms(&mut self, ms: u64) {
        self.st().now += ms * 1000;
    }

    fn spin_until(&mut self, t_us: u64) {
        let mut st = self.st();
        if st.now < t_us {
            st.now = t_us;
        }
    }
}
