//! OBD interface for serial K-line adapters (K+DCAN / ADS style cables),
//! implementing `best::Interface`.
//!
//! Ported from EdiabasLib (`EdInterfaceObd.cs`, GPL-3.0), serial port path
//! only: DS2 (concepts 1, 5, 6), KWP2000 BMW (0x010C), KWP2000* (0x010D),
//! BMW-FAST (0x010F, D-CAN 0x0110 over the K-line telegram format) and ISO
//! KWP2000 with 5 baud init (C#: EDIC concept 0 only). Not ported:
//! Bluetooth/ELM/WiFi/FTDI D2XX adapters, KWP1281, concept 3, ISO-TP, TP2.0,
//! EDIC, bit-bang, voltage sampler, transmit cache, simulation (see
//! `best::sim`).

pub mod core;
pub mod iface;
pub mod link;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use iface::ObdInterface;
pub use link::{Parity, SerialLink, SerialPortLink};
