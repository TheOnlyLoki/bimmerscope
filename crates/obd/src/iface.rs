//! `ObdInterface`: `best::Interface` for a serial K-line adapter
//! (C#: `EdInterfaceObd`, serial port path, no simulation).

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use best::ErrorCode;
use best::iface::{IfResult, Interface};
use best::telegram::TRANS_BUFFER_SIZE;

use crate::core::{ObdCore, TICK_NEVER, ThreadCommand};
use crate::link::{Parity, SerialLink, SerialPortLink};

/// Opens the serial link for a port name.
pub type LinkOpener = Box<dyn Fn(&str) -> io::Result<Box<dyn SerialLink>> + Send>;

/// C#: `CommThread`: runs the idle (tester present) and frequent
/// transmissions every 10 ms while no request is active.
struct CommThread {
    handle: Option<JoinHandle<()>>,
    exit: Arc<AtomicBool>,
}

pub struct ObdInterface {
    core: Arc<Mutex<ObdCore>>,
    /// C#: `ComPortProtected`.
    port_name: String,
    opener: LinkOpener,
    /// `None`: comm thread not started (C#: `CommThread == null`).
    thread: Option<CommThread>,
    /// Spawn a real thread for idle transmissions. Off: the thread is only
    /// tracked logically (tests).
    use_comm_thread: bool,
    /// Copies for the borrowing getters of `Interface`.
    comm_par: Option<Vec<u32>>,
    comm_answer_len: [i16; 2],
}

fn lock(core: &Mutex<ObdCore>) -> MutexGuard<'_, ObdCore> {
    core.lock().unwrap_or_else(|e| e.into_inner())
}

impl ObdInterface {
    /// Interface for a serial port, e.g. `/dev/ttyUSB0`. The port is opened
    /// by `interface_connect`.
    pub fn new(port_name: &str) -> ObdInterface {
        ObdInterface::with_opener(
            port_name,
            Box::new(|name: &str| {
                SerialPortLink::open(name).map(|l| Box::new(l) as Box<dyn SerialLink>)
            }),
        )
    }

    /// Interface with a custom link (e.g. a mock).
    pub fn with_opener(port_name: &str, opener: LinkOpener) -> ObdInterface {
        ObdInterface {
            core: Arc::new(Mutex::new(ObdCore::new())),
            port_name: port_name.to_string(),
            opener,
            thread: None,
            use_comm_thread: true,
            comm_par: None,
            comm_answer_len: [0, 0],
        }
    }

    /// Telegram log, shared with the interface.
    pub fn log_handle(&self) -> Arc<Mutex<Vec<String>>> {
        lock(&self.core).log.clone()
    }

    /// Access to the protocol state (configuration fields, tests).
    pub fn core(&self) -> MutexGuard<'_, ObdCore> {
        lock(&self.core)
    }

    pub fn set_use_comm_thread(&mut self, enable: bool) {
        self.use_comm_thread = enable;
    }

    /// C#: adapter echo. True for a plain serial port (K+DCAN cable).
    pub fn set_has_adapter_echo(&mut self, echo: bool) {
        lock(&self.core).has_adapter_echo = echo;
    }

    /// C#: config `ObdAddRecTimeout` (ms, default 20).
    pub fn set_add_rec_timeout(&mut self, ms: i32) {
        lock(&self.core).add_rec_timeout = ms;
    }

    /// C#: config `ObdDtrTimeCorrCom` (ms, default 0.3).
    pub fn set_dtr_time_corr_com(&mut self, ms: f64) {
        lock(&self.core).dtr_time_corr_com = ms;
    }

    /// C#: config `RetryComm` (default on).
    pub fn set_retry_comm(&mut self, retry: bool) {
        lock(&self.core).retry_comm = retry;
    }

    /// C#: config `ObdDisabledConcepts`.
    pub fn set_disabled_concepts(&mut self, concepts: Vec<u32>) {
        lock(&self.core).disabled_concepts = concepts;
    }

    /// C#: `Ediabas.SgbdFileName` (KWP2000 protocol check).
    pub fn set_sgbd_file_name(&mut self, name: &str) {
        lock(&self.core).sgbd_file_name = name.to_string();
    }

    fn sync_copies(&mut self) {
        let c = lock(&self.core);
        self.comm_par = c.comm_par.clone();
        self.comm_answer_len = c.comm_answer_len;
    }

    /// C#: `StartCommThread`.
    fn start_comm_thread(&mut self) {
        if self.thread.is_some() {
            return;
        }
        {
            let mut c = lock(&self.core);
            c.log_str("Start comm thread");
            c.thread_command = ThreadCommand::Idle;
        }
        let exit = Arc::new(AtomicBool::new(false));
        let handle = if self.use_comm_thread {
            let core = self.core.clone();
            let exit = exit.clone();
            std::thread::Builder::new()
                .name("obd-comm".into())
                .spawn(move || {
                    loop {
                        std::thread::park_timeout(Duration::from_millis(10));
                        if exit.load(Ordering::SeqCst) {
                            break;
                        }
                        let mut c = lock(&core);
                        if c.thread_command == ThreadCommand::IdleTransmit {
                            c.idle_transmit();
                        }
                    }
                })
                .ok()
        } else {
            None
        };
        self.thread = Some(CommThread { handle, exit });
    }

    /// C#: `StopCommThread`: the thread runs `ObdFinishTrans` on exit.
    fn stop_comm_thread(&mut self) {
        if let Some(t) = self.thread.take() {
            t.exit.store(true, Ordering::SeqCst);
            if let Some(h) = t.handle {
                h.thread().unpark();
                let _ = h.join();
            }
            let mut c = lock(&self.core);
            c.log_str("Stop comm thread");
            let _ = c.obd_finish_trans();
            c.thread_command = ThreadCommand::Idle;
        }
        let mut c = lock(&self.core);
        if !c.send_buffer_frequent.is_empty() {
            c.log_str("Stopping frequent mode");
            c.send_buffer_frequent.clear();
            c.rec_buffer_frequent.clear();
        }
    }

    /// C#: `GetDsrState`.
    fn dsr_state(&mut self) -> IfResult<bool> {
        let mut c = lock(&self.core);
        match c.link.as_mut().map(|l| l.dsr()) {
            Some(Ok(v)) => Ok(v),
            _ => Err(ErrorCode::EDIABAS_IFH_0019),
        }
    }
}

impl Drop for ObdInterface {
    fn drop(&mut self) {
        self.stop_comm_thread();
    }
}

impl Interface for ObdInterface {
    fn interface_type(&self) -> &str {
        "OBD"
    }

    fn interface_version(&self) -> u32 {
        209
    }

    fn connected(&self) -> bool {
        lock(&self.core).link.is_some()
    }

    fn interface_connect(&mut self) -> IfResult<()> {
        {
            // C#: base `InterfaceConnect` sets the field, not the property.
            let mut c = lock(&self.core);
            c.comm_repeats = 0;
            c.comm_par = None;
            c.comm_answer_len = [0, 0];
            c.current_baud_rate = 9600;
            c.current_parity = Parity::None;
            c.current_data_bits = 8;
        }
        self.sync_copies();
        if self.port_name.is_empty() {
            return Err(ErrorCode::EDIABAS_IFH_0018);
        }
        let mut c = lock(&self.core);
        if c.link.is_some() {
            return Ok(());
        }
        match (self.opener)(&self.port_name) {
            Ok(link) => {
                c.link = Some(link);
                c.last_comm_tick = TICK_NEVER;
                c.log_str(&format!("Port {} opened", self.port_name));
                Ok(())
            }
            Err(e) => {
                c.log_str(&format!("*** Open {} failed: {e}", self.port_name));
                Err(ErrorCode::EDIABAS_IFH_0018)
            }
        }
    }

    fn interface_disconnect(&mut self) -> IfResult<()> {
        self.stop_comm_thread();
        let mut c = lock(&self.core);
        c.link = None;
        Ok(())
    }

    fn interface_reset(&mut self) -> IfResult<()> {
        let connected = self.connected();
        self.set_comm_parameter_none();
        if !connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(())
    }

    fn interface_boot(&mut self) -> IfResult<()> {
        if !lock(&self.core).send_buffer_frequent.is_empty() {
            // C# returns false without error code: ignored.
            lock(&self.core).log_str("Ignoring interface boot in frequent mode");
            return Ok(());
        }
        self.set_comm_parameter_none();
        Ok(())
    }

    fn transmit_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>> {
        if !lock(&self.core).send_buffer_frequent.is_empty() {
            lock(&self.core).log_str("Frequent mode active, stopping before transmission");
            self.stop_frequent()?;
        }
        {
            let c = lock(&self.core);
            if c.comm_par.is_none() {
                return Err(ErrorCode::EDIABAS_IFH_0006);
            }
        }
        if request.len() > TRANS_BUFFER_SIZE {
            return Err(ErrorCode::EDIABAS_IFH_0031);
        }
        self.start_comm_thread();
        lock(&self.core).single_transmit(request)
    }

    fn transmit_frequent(&mut self, request: &[u8]) -> IfResult<()> {
        {
            let c = lock(&self.core);
            c.log_data(request, "Send Frequent");
            if c.comm_par.is_none() {
                return Err(ErrorCode::EDIABAS_IFH_0006);
            }
            if !c.par_support_frequent {
                c.log_str("Frequent not supported");
                return Err(ErrorCode::EDIABAS_IFH_0006);
            }
            if request.is_empty() {
                c.log_str("No frequent send data");
                return Err(ErrorCode::EDIABAS_IFH_0006);
            }
        }
        self.start_comm_thread();
        lock(&self.core).frequent_mode(request)
    }

    fn receive_frequent(&mut self) -> IfResult<Vec<u8>> {
        {
            let c = lock(&self.core);
            if c.comm_par.is_none() {
                return Err(ErrorCode::EDIABAS_IFH_0006);
            }
            if !c.par_support_frequent {
                return Ok(Vec::new());
            }
        }
        self.start_comm_thread();
        let mut c = lock(&self.core);
        let r = c.single_transmit(&[]);
        if let Ok(data) = &r {
            c.log_data(data, "Frequent");
        }
        r
    }

    fn stop_frequent(&mut self) -> IfResult<()> {
        self.stop_comm_thread();
        lock(&self.core).log_str("Frequent mode stopped");
        Ok(())
    }

    fn raw_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>> {
        if request == [0xF1, 0x07] {
            return Ok(vec![0x05, 0x03, 0x00]);
        }
        Ok(Vec::new())
    }

    fn comm_parameter(&self) -> &[u32] {
        self.comm_par.as_deref().unwrap_or(&[])
    }

    fn set_comm_parameter(&mut self, pars: Vec<u32>) -> IfResult<()> {
        self.stop_comm_thread();
        let r = {
            let mut c = lock(&self.core);
            c.comm_par = Some(pars);
            c.apply_comm_parameter()
        };
        self.sync_copies();
        r
    }

    fn comm_answer_len(&self) -> &[i16] {
        &self.comm_answer_len
    }

    fn set_comm_answer_len(&mut self, len: Vec<i16>) -> IfResult<()> {
        if len.len() >= 2 {
            let mut c = lock(&self.core);
            c.comm_answer_len = [len[0], len[1]];
            c.log_str(&format!("Answer length: {:04X} {:04X}", len[0], len[1]));
        }
        self.sync_copies();
        Ok(())
    }

    fn comm_repeats(&self) -> u32 {
        lock(&self.core).comm_repeats
    }

    fn set_comm_repeats(&mut self, repeats: u32) {
        lock(&self.core).comm_repeats = repeats;
    }

    fn key_bytes(&mut self) -> IfResult<Vec<u8>> {
        if !self.connected() {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        self.start_comm_thread();
        let mut c = lock(&self.core);
        if c.comm_par.is_some() && c.par_has_key_bytes {
            if c.ecu_connected {
                return Ok(c.key_bytes.clone());
            }
            // start transmission (C# reuses the last send buffer length;
            // here: no request data)
            c.single_transmit(&[])?;
            return Ok(c.key_bytes.clone());
        }
        Ok(Vec::new())
    }

    fn state(&mut self) -> IfResult<Vec<u8>> {
        if !self.connected() {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        let dsr = self.dsr_state()?;
        Ok(vec![0x00, if dsr { 0x00 } else { 0x30 }])
    }

    fn loop_test(&mut self) -> IfResult<u32> {
        if !self.connected() {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(1)
    }

    fn battery_voltage(&mut self) -> IfResult<i64> {
        if !self.connected() {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        let dsr = self.dsr_state()?;
        Ok(if dsr {
            lock(&self.core).battery_voltage_value
        } else {
            0
        })
    }

    /// C#: no ignition status function on a plain serial port: DSR.
    fn ignition_voltage(&mut self) -> IfResult<i64> {
        if !self.connected() {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        let dsr = self.dsr_state()?;
        Ok(if dsr {
            lock(&self.core).ignition_voltage_value
        } else {
            0
        })
    }

    fn get_port(&mut self, _index: u32) -> IfResult<i64> {
        Ok(0)
    }

    fn set_port(&mut self, _index: u32, _value: u32) -> IfResult<()> {
        Ok(())
    }

    fn set_program_voltage(&mut self, _voltage: u32) -> IfResult<()> {
        Ok(())
    }

    fn switch_si_relais(&mut self, _time: u32) -> IfResult<()> {
        Ok(())
    }
}

impl ObdInterface {
    /// C#: `CommParameter = null`.
    fn set_comm_parameter_none(&mut self) {
        self.stop_comm_thread();
        {
            let mut c = lock(&self.core);
            c.comm_par = None;
            let _ = c.apply_comm_parameter();
        }
        self.sync_copies();
    }
}
