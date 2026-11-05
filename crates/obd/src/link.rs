//! Serial port abstraction (C#: the `System.IO.Ports.SerialPort` calls of
//! `EdInterfaceObd`), so the protocol code can run against a mock.

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

/// C#: `EdInterfaceObd.SerialParity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Odd,
    Even,
    Mark,
    Space,
}

/// One serial line plus a clock. All protocol timing goes through the
/// clock, so a mock can simulate time.
pub trait SerialLink: Send {
    /// Baud rate, data bits and parity (C#: `SerialPort.BaudRate`,
    /// `DataBits`, `Parity`). Implementations skip values that are already set.
    fn set_config(&mut self, baud_rate: u32, data_bits: u8, parity: Parity) -> io::Result<()>;
    /// C#: `SerialPort.DtrEnable`.
    fn set_dtr(&mut self, on: bool) -> io::Result<()>;
    /// C#: `SerialPort.RtsEnable`.
    fn set_rts(&mut self, on: bool) -> io::Result<()>;
    /// C#: `SerialPort.DsrHolding`.
    fn dsr(&mut self) -> io::Result<bool>;
    /// C#: `SerialPort.BreakState`.
    fn set_break(&mut self, on: bool) -> io::Result<()>;
    /// C#: `SerialPort.DiscardInBuffer`.
    fn purge_input(&mut self) -> io::Result<()>;
    /// C#: `SerialPort.Write`. Returns once the bytes are handed to the driver.
    fn write(&mut self, data: &[u8]) -> io::Result<()>;
    /// C#: `SerialPort.BytesToRead`.
    fn bytes_to_read(&mut self) -> io::Result<usize>;
    /// C#: `SerialPort.Read`: reads available bytes, at most `buf.len()`.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    /// C#: `CommReceiveEvent.WaitOne(ms)`: waits up to `ms` for received data.
    fn wait_rx(&mut self, ms: u64);
    /// Monotonic time in microseconds (C#: `Stopwatch.GetTimestamp`).
    fn now_us(&self) -> u64;
    /// C#: `Thread.Sleep(ms)`.
    fn sleep_ms(&mut self, ms: u64);
    /// Busy wait until `now_us() >= t_us` (C#: `while (Stopwatch... < x) {}`).
    fn spin_until(&mut self, t_us: u64);
}

/// Real serial port, e.g. `/dev/ttyUSB0` of a K+DCAN cable (FTDI, `ftdi_sio`).
pub struct SerialPortLink {
    port: Box<dyn serialport::SerialPort>,
    start: Instant,
    baud_rate: u32,
    data_bits: u8,
    parity: Parity,
}

impl SerialPortLink {
    /// C#: `InterfaceConnect` port setup: 9600 8N1, no handshake, DTR and
    /// RTS off, read timeout 1 ms.
    pub fn open(port_name: &str) -> io::Result<SerialPortLink> {
        let mut port = serialport::new(port_name, 9600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(1))
            .dtr_on_open(false)
            .open()?;
        port.write_data_terminal_ready(false)?;
        port.write_request_to_send(false)?;
        Ok(SerialPortLink { port, start: Instant::now(), baud_rate: 9600, data_bits: 8, parity: Parity::None })
    }
}

impl SerialLink for SerialPortLink {
    fn set_config(&mut self, baud_rate: u32, data_bits: u8, parity: Parity) -> io::Result<()> {
        if self.baud_rate != baud_rate {
            self.port.set_baud_rate(baud_rate)?;
            self.baud_rate = baud_rate;
        }
        if self.data_bits != data_bits {
            let bits = match data_bits {
                5 => serialport::DataBits::Five,
                6 => serialport::DataBits::Six,
                7 => serialport::DataBits::Seven,
                8 => serialport::DataBits::Eight,
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, "data bits")),
            };
            self.port.set_data_bits(bits)?;
            self.data_bits = data_bits;
        }
        if self.parity != parity {
            let p = match parity {
                Parity::None => serialport::Parity::None,
                Parity::Odd => serialport::Parity::Odd,
                Parity::Even => serialport::Parity::Even,
                Parity::Mark | Parity::Space => {
                    return Err(io::Error::new(io::ErrorKind::Unsupported, "mark/space parity"));
                }
            };
            self.port.set_parity(p)?;
            self.parity = parity;
        }
        Ok(())
    }

    fn set_dtr(&mut self, on: bool) -> io::Result<()> {
        Ok(self.port.write_data_terminal_ready(on)?)
    }

    fn set_rts(&mut self, on: bool) -> io::Result<()> {
        Ok(self.port.write_request_to_send(on)?)
    }

    fn dsr(&mut self) -> io::Result<bool> {
        Ok(self.port.read_data_set_ready()?)
    }

    fn set_break(&mut self, on: bool) -> io::Result<()> {
        if on {
            Ok(self.port.set_break()?)
        } else {
            Ok(self.port.clear_break()?)
        }
    }

    fn purge_input(&mut self) -> io::Result<()> {
        Ok(self.port.clear(serialport::ClearBuffer::Input)?)
    }

    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.port.write_all(data)
    }

    fn bytes_to_read(&mut self) -> io::Result<usize> {
        Ok(self.port.bytes_to_read()? as usize)
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }

    fn wait_rx(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn now_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }

    fn spin_until(&mut self, t_us: u64) {
        while self.now_us() < t_us {
            std::hint::spin_loop();
        }
    }
}
