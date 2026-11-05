//! Protocol state and handlers of the serial K-line path of
//! `EdInterfaceObd.cs` (EdiabasLib, GPL-3.0). Names follow the C# source.
//!
//! Runs single threaded on a [`SerialLink`]; `ObdInterface` adds the comm
//! thread around it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use best::ErrorCode;
use best::iface::IfResult;
use best::telegram::{
    TRANS_BUFFER_SIZE, calc_checksum_bmw_fast, calc_checksum_xor, tel_length_bmw_fast, tel_length_kwp2000s,
};

use crate::link::{Parity, SerialLink};

/// C#: `DefaultStdTimeout` (60 s).
pub const DEFAULT_STD_TIMEOUT: i32 = 60000;
/// C#: `EchoTimeout`.
pub const ECHO_TIMEOUT: i32 = 100;
/// C#: `Kwp1281InitLongDelay`.
pub const KWP1281_INIT_LONG_DELAY: i32 = 2600;
/// C#: `Kwp1281InitShortDelay`.
pub const KWP1281_INIT_SHORT_DELAY: i32 = 1500;
/// C#: size of `Kwp1281Buffer`, the receive buffer of the idle telegrams.
const KWP1281_BUFFER_SIZE: usize = 256;
/// Max. telegram log entries kept.
const LOG_MAX: usize = 20000;
/// C#: `DateTime.MinValue.Ticks` as start value of the tick fields: long ago.
pub const TICK_NEVER: i64 = i64::MIN / 4;

/// C#: `ParTransmitFunc` (only the K-line handlers are ported).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transmit {
    /// Concept 1, DS1 (5), DS2 (6): `TransDs2`.
    Ds2,
    /// Concept 0x010C: `TransKwp2000Bmw`.
    Kwp2000Bmw,
    /// Concept 0x010D: `TransKwp2000S`.
    Kwp2000S,
    /// Concept 0x010F, D-CAN 0x0110: `TransBmwFast`.
    BmwFast,
    /// ISO 14230 with 5 baud init: `TransKwp2000` (C#: EDIC concept 0 only).
    Kwp2000,
}

/// C#: `KwpModes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KwpMode {
    Undefined,
    Kwp2000,
    Kwp1281,
}

/// C#: `CommThreadCommands` that survive a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadCommand {
    Idle,
    IdleTransmit,
}

pub struct ObdCore {
    /// C#: `SerialPort` (`None` = closed).
    pub link: Option<Box<dyn SerialLink>>,
    /// Telegram log (C#: `LogData`/`LogString` at level Ifh).
    pub log: Arc<Mutex<Vec<String>>>,

    // Configuration (C#: config properties and adapter properties).
    /// C#: `HasAdapterEcho`. A plain serial port (K+DCAN cable) has echo.
    pub has_adapter_echo: bool,
    /// C#: `HasAutoBaudRate`, true for a plain serial port.
    pub has_auto_baud_rate: bool,
    /// C#: `DtrTimeCorrCom` (ms).
    pub dtr_time_corr_com: f64,
    /// C#: `AddRecTimeout` (ms).
    pub add_rec_timeout: i32,
    /// C#: `BatteryVoltageValue` (mV).
    pub battery_voltage_value: i64,
    /// C#: `IgnitionVoltageValue` (mV).
    pub ignition_voltage_value: i64,
    /// C#: config `RetryComm` != 0.
    pub retry_comm: bool,
    /// C#: `Ediabas.SgbdFileName`, used by the KWP2000 protocol check.
    pub sgbd_file_name: String,
    /// C#: `DisabledConceptsList` (config `ObdDisabledConcepts`).
    pub disabled_concepts: Vec<u32>,

    // C#: EdInterfaceBase.
    pub comm_par: Option<Vec<u32>>,
    pub comm_answer_len: [i16; 2],
    pub comm_repeats: u32,

    pub thread_command: ThreadCommand,

    pub transmit: Option<Transmit>,
    pub par_timeout_std: i32,
    pub par_timeout_tel_end: i32,
    pub par_interbyte_time: i32,
    pub par_regen_time: i32,
    pub par_request_time_nr21: i32,
    pub par_request_time_nr23: i32,
    pub par_retry_nr21: i32,
    pub par_retry_nr23: i32,
    pub par_timeout_nr78: i32,
    pub par_retry_nr78: i32,
    pub par_wake_address: u8,
    pub par_tester_present_time: i32,
    pub par_tester_present_tel: Vec<u8>,
    pub par_start_comm_tel: Vec<u8>,
    pub par_checksum_by_user: bool,
    pub par_checksum_no_check: bool,
    pub par_send_set_dtr: bool,
    pub par_has_key_bytes: bool,
    pub par_support_frequent: bool,

    // C#: ParEdic* (ISO KWP2000 with 5 baud init).
    pub par_edic_wake_address: u8,
    pub par_edic_tester_address: u8,
    pub par_edic_ecu_address: u8,
    pub par_edic_w1: i32,
    pub par_edic_w2: i32,
    pub par_edic_w3: i32,
    pub par_edic_w4a: i32,
    pub par_edic_w4: i32,
    pub par_edic_tester_present_time: i32,
    pub par_edic_tester_present_tel: Vec<u8>,
    pub par_edic_init_delay: i32,
    pub par_edic_add_retries: i32,

    pub key_bytes: Vec<u8>,
    pub send_buffer_frequent: Vec<u8>,
    pub rec_buffer_frequent: Vec<u8>,
    pub nr78_dict: HashMap<u8, i32>,
    pub ecu_connected: bool,
    pub kwp_mode: KwpMode,
    /// Link time in µs.
    pub last_comm_tick: i64,
    pub last_response_tick: i64,
    pub current_baud_rate: u32,
    pub current_parity: Parity,
    pub current_data_bits: u8,
    pub valid_kline_response: bool,
}

impl Default for ObdCore {
    fn default() -> Self {
        ObdCore::new()
    }
}

impl ObdCore {
    pub fn new() -> ObdCore {
        ObdCore {
            link: None,
            log: Arc::new(Mutex::new(Vec::new())),
            has_adapter_echo: true,
            has_auto_baud_rate: true,
            dtr_time_corr_com: 0.3,
            add_rec_timeout: 20,
            battery_voltage_value: 12000,
            ignition_voltage_value: 12000,
            retry_comm: true,
            sgbd_file_name: String::new(),
            disabled_concepts: Vec::new(),
            comm_par: None,
            comm_answer_len: [0, 0],
            comm_repeats: 0,
            thread_command: ThreadCommand::Idle,
            transmit: None,
            par_timeout_std: 0,
            par_timeout_tel_end: 0,
            par_interbyte_time: 0,
            par_regen_time: 0,
            par_request_time_nr21: 0,
            par_request_time_nr23: 0,
            par_retry_nr21: 0,
            par_retry_nr23: 0,
            par_timeout_nr78: 0,
            par_retry_nr78: 0,
            par_wake_address: 0,
            par_tester_present_time: 0,
            par_tester_present_tel: Vec::new(),
            par_start_comm_tel: Vec::new(),
            par_checksum_by_user: false,
            par_checksum_no_check: false,
            par_send_set_dtr: false,
            par_has_key_bytes: false,
            par_support_frequent: false,
            par_edic_wake_address: 0,
            par_edic_tester_address: 0,
            par_edic_ecu_address: 0,
            par_edic_w1: 0,
            par_edic_w2: 0,
            par_edic_w3: 0,
            par_edic_w4a: 0,
            par_edic_w4: 0,
            par_edic_tester_present_time: 0,
            par_edic_tester_present_tel: Vec::new(),
            par_edic_init_delay: 0,
            par_edic_add_retries: 0,
            key_bytes: Vec::new(),
            send_buffer_frequent: Vec::new(),
            rec_buffer_frequent: Vec::new(),
            nr78_dict: HashMap::new(),
            ecu_connected: false,
            kwp_mode: KwpMode::Undefined,
            last_comm_tick: TICK_NEVER,
            last_response_tick: TICK_NEVER,
            current_baud_rate: 9600,
            current_parity: Parity::None,
            current_data_bits: 8,
            valid_kline_response: false,
        }
    }

    // -----------------------------------------------------------------------
    // Time and logging
    // -----------------------------------------------------------------------

    fn now(&self) -> i64 {
        self.link.as_ref().map_or(0, |l| l.now_us() as i64)
    }

    /// C#: `(Stopwatch.GetTimestamp() - tick) < ms * TickResolMs`.
    fn within_ms(&self, tick: i64, ms: i32) -> bool {
        self.now().saturating_sub(tick) < ms as i64 * 1000
    }

    fn sleep_ms(&mut self, ms: i32) {
        if let Some(l) = self.link.as_mut() {
            l.sleep_ms(ms.max(0) as u64);
        }
    }

    pub fn log_str(&self, text: &str) {
        let line = format!("[{:>10.3}] {}", self.now() as f64 / 1000.0, text);
        let mut log = self.log.lock().unwrap_or_else(|e| e.into_inner());
        if log.len() >= LOG_MAX {
            log.remove(0);
        }
        log.push(line);
    }

    pub fn log_data(&self, data: &[u8], info: &str) {
        let hex: Vec<String> = data.iter().map(|b| format!("{b:02X}")).collect();
        self.log_str(&format!("{info}: {}", hex.join(" ")));
    }

    // -----------------------------------------------------------------------
    // CommParameter (C#: `CommParameter` setter)
    // -----------------------------------------------------------------------

    /// C#: `EvalChecksumPar`.
    fn eval_checksum_par(&mut self, value: u32) -> bool {
        self.par_checksum_by_user = (value & 0x01) == 0;
        self.par_checksum_no_check = (value & 0x02) != 0;
        true
    }

    /// C#: `CommParameter` setter after `StopCommThread`, for `comm_par`.
    pub fn apply_comm_parameter(&mut self) -> IfResult<()> {
        let edic_par = self.comm_par.as_ref().is_some_and(|p| p.first() == Some(&0x0000));

        self.comm_answer_len = [0, 0];
        self.transmit = None;
        if !edic_par {
            self.par_timeout_std = 0;
            self.par_timeout_tel_end = 0;
            self.par_interbyte_time = 0;
            self.par_regen_time = 0;
            self.par_request_time_nr21 = 0;
            self.par_request_time_nr23 = 0;
            self.par_retry_nr21 = 0;
            self.par_retry_nr23 = 0;
            self.par_timeout_nr78 = 0;
            self.par_retry_nr78 = 0;
        }
        self.par_wake_address = 0;
        self.par_tester_present_time = 0;
        self.par_tester_present_tel.clear();
        self.par_start_comm_tel.clear();
        self.par_checksum_by_user = false;
        self.par_checksum_no_check = false;
        self.par_send_set_dtr = false;
        self.par_has_key_bytes = false;
        self.par_support_frequent = false;
        self.key_bytes.clear();
        self.send_buffer_frequent.clear();
        self.rec_buffer_frequent.clear();
        self.nr78_dict.clear();
        self.ecu_connected = false;
        self.kwp_mode = KwpMode::Undefined;
        // don't init last_comm_tick here
        self.last_response_tick = TICK_NEVER;

        let Some(p) = self.comm_par.clone() else {
            // clear parameter
            return Ok(());
        };
        if p.is_empty() {
            self.log_str(&format!("*** Parameter length too short: {}", p.len()));
            return Err(ErrorCode::EDIABAS_IFH_0041);
        }
        let pars: Vec<String> = p.iter().map(|v| format!("{v:X}")).collect();
        self.log_str(&format!("CommParameter: {}", pars.join(" ")));

        let mut check_std_timeout = false;
        let baud_rate: u32;
        let data_bits: u8 = 8;
        let parity: Parity;
        let mut state_dtr = false;
        let state_rts = false;
        let concept = p[0];

        if self.disabled_concepts.contains(&concept) {
            self.log_str(&format!("*** Concept {concept:04X} disabled"));
            return Err(ErrorCode::EDIABAS_IFH_0014);
        }

        match concept {
            0x0000 => {
                // Raw (EDIC): EdicSimulation is false for the OBD interface.
                self.log_str("*** No EDIC simulation mode");
                return Err(ErrorCode::EDIABAS_IFH_0014);
            }
            0x0001 => {
                if self.has_adapter_echo {
                    // only with ADS adapter
                    return Err(ErrorCode::EDIABAS_IFH_0006);
                }
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                if p.len() >= 10 && !self.eval_checksum_par(p[9]) {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.comm_answer_len = [-2, 0];
                check_std_timeout = true;
                baud_rate = p[1];
                parity = Parity::Even;
                self.transmit = Some(Transmit::Ds2);
                self.par_timeout_std = p[5] as i32;
                self.par_regen_time = p[6] as i32;
                // C# quirk: reads index 7 with only 7 parameters checked.
                self.par_timeout_tel_end = p.get(7).copied().unwrap_or(0) as i32;
                self.par_send_set_dtr = false;
                self.ecu_connected = true;
                self.par_support_frequent = true;
            }
            0x0002 | 0x0003 => {
                if self.has_adapter_echo {
                    // only with ADS adapter
                    return Err(ErrorCode::EDIABAS_IFH_0006);
                }
                // KWP1281 / concept 3 are not ported.
                self.log_str(&format!("*** Concept not implemented: {concept:04X}"));
                return Err(ErrorCode::EDIABAS_IFH_0014);
            }
            0x0005 | 0x0006 => {
                // DS1, DS2
                if p.len() < 8 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                if p.len() >= 10 && !self.eval_checksum_par(p[9]) {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.comm_answer_len = [-1, 0];
                check_std_timeout = true;
                baud_rate = p[1];
                parity = Parity::Even;
                self.transmit = Some(Transmit::Ds2);
                self.par_timeout_std = p[5] as i32;
                self.par_regen_time = p[6] as i32;
                self.par_timeout_tel_end = p[7] as i32;
                if p.len() >= 9 {
                    self.par_interbyte_time = p[8] as i32;
                }
                self.par_send_set_dtr = false;
                if concept == 6 {
                    // DS2 uses DTR
                    self.par_send_set_dtr = !self.has_adapter_echo;
                }
                self.ecu_connected = true;
                self.par_support_frequent = true;
            }
            0x010B => {
                // KWP2000 Standard (motorbikes)
                self.log_str("*** Concept KWP2000 standard not implemented");
                return Err(ErrorCode::EDIABAS_IFH_0014);
            }
            0x010C => {
                // KWP2000 BMW
                if p.len() < 33 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                if p.len() >= 34 && !self.eval_checksum_par(p[33]) {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                baud_rate = p[1];
                parity = Parity::None;
                self.transmit = Some(Transmit::Kwp2000Bmw);
                self.par_timeout_std = p[2] as i32;
                self.par_regen_time = p[3] as i32;
                self.par_timeout_tel_end = p[4] as i32;
                self.par_interbyte_time = p[5] as i32;
                self.par_timeout_nr78 = p[7] as i32;
                self.par_retry_nr78 = p[6] as i32;
                self.par_tester_present_time = p[8] as i32;
                let tp_len = p[9] as i32;
                if tp_len > 11 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.par_tester_present_tel = p[10..10 + tp_len as usize].iter().map(|v| *v as u8).collect();
                let sc_len = p[21] as i32;
                if sc_len > 11 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.par_start_comm_tel = p[22..22 + sc_len as usize].iter().map(|v| *v as u8).collect();
                self.par_send_set_dtr = !self.has_adapter_echo;
                self.ecu_connected = true;
                self.par_support_frequent = true;
            }
            0x010D => {
                // KWP2000*
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                if p.len() >= 22 && !self.eval_checksum_par(p[21]) {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                baud_rate = p[1];
                parity = Parity::Even;
                self.transmit = Some(Transmit::Kwp2000S);
                self.par_timeout_std = p[2] as i32;
                self.par_regen_time = p[3] as i32;
                self.par_timeout_tel_end = p[4] as i32;
                self.par_interbyte_time = p[5] as i32;
                // C# quirk: reads index 7 with only 7 parameters checked.
                self.par_timeout_nr78 = p.get(7).copied().unwrap_or(0) as i32;
                self.par_retry_nr78 = p[6] as i32;
                self.par_send_set_dtr = !self.has_adapter_echo;
            }
            0x010F => {
                // BMW-FAST
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                if p.len() >= 8 && !self.eval_checksum_par(p[7]) {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                baud_rate = p[1];
                parity = Parity::None;
                state_dtr = self.has_adapter_echo;
                self.transmit = Some(Transmit::BmwFast);
                self.par_timeout_std = p[2] as i32;
                self.par_regen_time = p[3] as i32;
                self.par_timeout_tel_end = p[4] as i32;
                self.par_timeout_nr78 = p[6] as i32;
                self.par_retry_nr78 = p[5] as i32;
                self.par_send_set_dtr = !self.has_adapter_echo;
            }
            0x0110 => {
                // D-CAN
                if p.len() < 30 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                baud_rate = 115200;
                parity = Parity::None;
                state_dtr = self.has_adapter_echo;
                self.transmit = Some(Transmit::BmwFast);
                self.par_timeout_std = p[7] as i32;
                self.par_timeout_tel_end = 10;
                self.par_regen_time = p[8] as i32;
                self.par_timeout_nr78 = p[9] as i32;
                self.par_retry_nr78 = p[10] as i32;
                self.par_send_set_dtr = !self.has_adapter_echo;
            }
            _ => {
                self.log_str(&format!("*** Concept not implemented: {concept:04X}"));
                return Err(ErrorCode::EDIABAS_IFH_0014);
            }
        }

        if check_std_timeout && self.par_timeout_std <= 0 {
            self.par_timeout_std = DEFAULT_STD_TIMEOUT;
            self.log_str(&format!("No timeout specified, using: {}", self.par_timeout_std));
        }

        if let Some(link) = self.link.as_mut() {
            let r = link
                .set_config(baud_rate, data_bits, parity)
                .and_then(|_| link.set_dtr(state_dtr))
                .and_then(|_| link.set_rts(state_rts));
            if r.is_err() {
                self.current_baud_rate = baud_rate;
                self.current_parity = parity;
                self.current_data_bits = data_bits;
                return Err(ErrorCode::EDIABAS_IFH_0041);
            }
        }
        self.current_baud_rate = baud_rate;
        self.current_parity = parity;
        self.current_data_bits = data_bits;
        Ok(())
    }

    pub fn has_idle_func(&self) -> bool {
        matches!(self.transmit, Some(Transmit::Kwp2000Bmw | Transmit::Kwp2000))
    }

    pub fn has_frequent_func(&self) -> bool {
        matches!(self.transmit, Some(Transmit::Ds2 | Transmit::Kwp2000Bmw | Transmit::Kwp2000))
    }

    // -----------------------------------------------------------------------
    // Comm thread requests (C#: `CommThreadFunc` branches)
    // -----------------------------------------------------------------------

    fn update_thread_command(&mut self) {
        self.thread_command = if !self.send_buffer_frequent.is_empty() || self.has_idle_func() {
            ThreadCommand::IdleTransmit
        } else {
            ThreadCommand::Idle
        };
    }

    /// C#: `CommThreadCommands.SingleTransmit`.
    pub fn single_transmit(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        let result = if !self.send_buffer_frequent.is_empty() {
            if self.rec_buffer_frequent.is_empty() {
                // errors only switch the thread to idle, which follows below anyway
                let _ = self.obd_frequent_trans();
            }
            if self.ecu_connected && !self.rec_buffer_frequent.is_empty() {
                Ok(std::mem::take(&mut self.rec_buffer_frequent))
            } else {
                Ok(Vec::new())
            }
        } else {
            self.obd_trans(send)
        };
        self.update_thread_command();
        result
    }

    /// C#: `CommThreadCommands.FrequentMode`.
    pub fn frequent_mode(&mut self, send: &[u8]) -> IfResult<()> {
        let mut result = Ok(());
        if send.is_empty() {
            // stop frequent
            self.send_buffer_frequent.clear();
            self.rec_buffer_frequent.clear();
        } else if !self.has_frequent_func() {
            result = Err(ErrorCode::EDIABAS_IFH_0006);
        } else {
            self.send_buffer_frequent = send.to_vec();
            self.rec_buffer_frequent.clear();
        }
        self.update_thread_command();
        result
    }

    /// C#: `CommThreadCommands.IdleTransmit`, one cycle.
    pub fn idle_transmit(&mut self) {
        let r = if self.send_buffer_frequent.is_empty() { self.obd_idle_trans() } else { self.obd_frequent_trans() };
        if r.is_err() && self.thread_command == ThreadCommand::IdleTransmit {
            self.thread_command = ThreadCommand::Idle;
        }
    }

    // -----------------------------------------------------------------------
    // Serial helpers
    // -----------------------------------------------------------------------

    fn bit_count(&self) -> u32 {
        if self.current_parity == Parity::None { self.current_data_bits as u32 + 2 } else { self.current_data_bits as u32 + 3 }
    }

    /// C#: `SendData(sendData, length, setDtr, interbyteTime)`.
    pub fn send_data_ibt(&mut self, data: &[u8], set_dtr: bool, interbyte_time: i32) -> bool {
        if data.is_empty() {
            return true;
        }
        if interbyte_time > 0 {
            let byte_time = 1.0 / self.current_baud_rate as f64 * 1000.0 * self.bit_count() as f64;
            let interbyte_us = ((interbyte_time as f64 + byte_time) * 1000.0) as u64;
            {
                let Some(link) = self.link.as_mut() else { return false };
                if link.purge_input().is_err() {
                    return false;
                }
            }
            if set_dtr && !self.set_dtr_signal(true) {
                return false;
            }
            for (i, b) in data.iter().enumerate() {
                let dtr_handling = (i + 1) >= data.len() && set_dtr;
                let start = self.now() as u64;
                if !self.send_data(&[*b], dtr_handling, true) {
                    return false;
                }
                if let Some(link) = self.link.as_mut() {
                    link.spin_until(start + interbyte_us);
                }
            }
            true
        } else {
            self.send_data(data, set_dtr, false)
        }
    }

    /// C#: `SendData(sendData, length, setDtr, keepInBuffer)`, serial port path.
    pub fn send_data(&mut self, data: &[u8], set_dtr: bool, keep_in_buffer: bool) -> bool {
        if data.is_empty() {
            return true;
        }
        let byte_time = 1.0 / self.current_baud_rate as f64 * 1000.0 * self.bit_count() as f64;
        let dtr_time_corr = self.dtr_time_corr_com;
        let Some(link) = self.link.as_mut() else { return false };
        let r: std::io::Result<()> = (|| {
            if set_dtr {
                let wait_us = ((dtr_time_corr + byte_time * data.len() as f64) * 1000.0) as u64;
                if !keep_in_buffer {
                    link.purge_input()?;
                }
                link.set_dtr(true)?;
                let start = link.now_us();
                link.write(data)?;
                link.spin_until(start + wait_us);
                link.set_dtr(false)?;
            } else {
                let wait_ms = (byte_time * data.len() as f64) as u64;
                if !keep_in_buffer {
                    link.purge_input()?;
                }
                link.write(data)?;
                if wait_ms > 10 {
                    link.sleep_ms(wait_ms);
                }
            }
            Ok(())
        })();
        r.is_ok()
    }

    /// C#: `ReceiveData`, serial port path. Reads exactly `length` bytes to
    /// `buf[offset..]`: `timeout` for the first byte, `timeout_tel_end` between
    /// bytes. Quirk: bytes are only read once all `length` bytes are buffered.
    pub fn receive_data(
        &mut self,
        buf: &mut [u8],
        offset: usize,
        length: usize,
        timeout: i32,
        timeout_tel_end: i32,
        log_response: bool,
    ) -> bool {
        if length == 0 {
            return true;
        }
        // add extra delay for internal signal transitions
        let timeout = (timeout + self.add_rec_timeout) as i64;
        let timeout_tel_end = (timeout_tel_end + self.add_rec_timeout) as i64;
        if offset + length > buf.len() {
            // C#: index exception, caught
            return false;
        }
        let Some(link) = self.link.as_mut() else { return false };

        // wait for first byte
        let start = link.now_us();
        let mut last_bytes_to_read;
        loop {
            last_bytes_to_read = match link.bytes_to_read() {
                Ok(n) => n,
                Err(_) => return false,
            };
            if last_bytes_to_read > 0 {
                break;
            }
            if ((link.now_us() - start) / 1000) as i64 > timeout {
                return false;
            }
            link.wait_rx(1);
        }

        let mut rec_len = 0;
        let mut start = link.now_us();
        loop {
            let bytes_to_read = match link.bytes_to_read() {
                Ok(n) => n,
                Err(_) => return false,
            };
            if bytes_to_read >= length {
                match link.read(&mut buf[offset + rec_len..offset + length]) {
                    Ok(n) => rec_len += n,
                    Err(_) => return false,
                }
            }
            if rec_len >= length {
                break;
            }
            if last_bytes_to_read != bytes_to_read {
                // bytes received
                start = link.now_us();
                last_bytes_to_read = bytes_to_read;
            } else if ((link.now_us() - start) / 1000) as i64 > timeout_tel_end {
                break;
            }
            link.wait_rx(1);
        }
        if log_response {
            self.log_data(&buf[offset..offset + rec_len], "Rec ");
        }
        rec_len >= length
    }

    /// C#: `ReceiveData(receiveData, 0, receiveData.Length, ...)`: waits until
    /// the line is quiet (always fails, the bytes stay in the input buffer).
    fn flush_receive(&mut self, buf: &mut [u8], timeout: i32, log: bool) {
        let len = buf.len();
        self.receive_data(buf, 0, len, timeout, self.par_timeout_tel_end, log);
    }

    /// C#: `SetDtrSignal`. Port errors are ignored.
    pub fn set_dtr_signal(&mut self, value: bool) -> bool {
        if let Some(link) = self.link.as_mut() {
            let _ = link.set_dtr(value);
        }
        true
    }

    /// C#: `SendWakeFastInit`: 25 ms break (K-line low), then 25 ms idle.
    pub fn send_wake_fast_init(&mut self, set_dtr: bool) -> bool {
        let Some(link) = self.link.as_mut() else { return false };
        let r: std::io::Result<()> = (|| {
            if set_dtr {
                link.set_dtr(true)?;
            }
            let start = link.now_us();
            link.set_break(true)?;
            link.spin_until(start + 25_000);
            link.set_break(false)?;
            link.spin_until(start + 50_000);
            if set_dtr {
                link.set_dtr(false)?;
            }
            Ok(())
        })();
        r.is_ok()
    }

    /// C#: `SendWakeAddress5Baud`: address bits at 5 baud via break state.
    /// `set_dtr`, `both_lines` and `auto_key_byte_delay` are only used by
    /// adapters with a pulse function, not by the serial port.
    pub fn send_wake_address_5baud(&mut self, value: u8, _set_dtr: bool, _both_lines: bool, _auto_key_byte_delay: i32) -> bool {
        let Some(link) = self.link.as_mut() else { return false };
        let r: std::io::Result<()> = (|| {
            link.purge_input()?;
            let start = link.now_us();
            link.set_break(true)?; // start bit
            link.sleep_ms(180);
            link.spin_until(start + 200_000);
            for i in 0..8u64 {
                link.set_break((value & (1 << i)) == 0)?;
                link.sleep_ms(180);
                link.spin_until(start + 200_000 * (i + 2));
            }
            link.set_break(false)?; // stop bit
            link.sleep_ms(200);
            Ok(())
        })();
        r.is_ok()
    }

    // -----------------------------------------------------------------------
    // ObdTrans
    // -----------------------------------------------------------------------

    /// C#: `ObdTrans`: transmit with `comm_repeats` retries.
    pub fn obd_trans(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        if self.link.is_none() {
            return Err(ErrorCode::EDIABAS_IFH_0019);
        }
        let Some(transmit) = self.transmit else {
            return Err(ErrorCode::EDIABAS_IFH_0006);
        };
        let retries = if self.retry_comm { self.comm_repeats } else { 0 };
        let mut error = ErrorCode::EDIABAS_ERR_NONE;
        for _ in 0..retries as u64 + 1 {
            let r = match transmit {
                Transmit::Ds2 => self.trans_ds2(send),
                Transmit::Kwp2000Bmw => self.trans_kwp2000_bmw(send),
                Transmit::Kwp2000S => self.trans_kwp2000s(send),
                Transmit::BmwFast => self.trans_kwp2000_tel(send, TRANS_BUFFER_SIZE, true),
                Transmit::Kwp2000 => self.trans_kwp2000_iso(send),
            };
            match r {
                Ok(v) => return Ok(v),
                Err(e) => error = e,
            }
            if error == ErrorCode::EDIABAS_IFH_0003 || error == ErrorCode::EDIABAS_IFH_0011 {
                // interface error / unknown interface
                break;
            }
            if send.is_empty() {
                // no data to send
                break;
            }
        }
        if error == ErrorCode::EDIABAS_IFH_0011 {
            // hide interface error
            error = ErrorCode::EDIABAS_IFH_0010;
        }
        Err(error)
    }

    /// C#: `ObdIdleTrans`.
    pub fn obd_idle_trans(&mut self) -> IfResult<()> {
        match self.transmit {
            Some(Transmit::Kwp2000Bmw) => self.idle_kwp2000_bmw(),
            Some(Transmit::Kwp2000) => self.idle_kwp2000(),
            _ => Err(ErrorCode::EDIABAS_IFH_0014),
        }
    }

    /// C#: `ObdFrequentTrans`.
    pub fn obd_frequent_trans(&mut self) -> IfResult<()> {
        match self.transmit {
            Some(Transmit::Ds2) => self.frequent_ds2(),
            Some(Transmit::Kwp2000Bmw) => self.frequent_kwp2000_bmw(),
            Some(Transmit::Kwp2000) => self.frequent_kwp2000(),
            _ => Err(ErrorCode::EDIABAS_IFH_0014),
        }
    }

    /// C#: `ObdFinishTrans`.
    pub fn obd_finish_trans(&mut self) -> IfResult<()> {
        match self.transmit {
            Some(Transmit::Kwp2000) => self.finish_kwp2000(),
            _ => Err(ErrorCode::EDIABAS_IFH_0014),
        }
    }

    fn nr78_dict_add(&mut self, device_addr: u8, enable_logging: bool) {
        if let Some(retries) = self.nr78_dict.remove(&device_addr) {
            let retries = retries + 1;
            if retries <= self.par_retry_nr78 {
                if enable_logging {
                    self.log_str(&format!("NR78({device_addr:02X}) count={retries}"));
                }
                self.nr78_dict.insert(device_addr, retries);
            } else if enable_logging {
                self.log_str(&format!("*** NR78({device_addr:02X}) exceeded"));
            }
        } else {
            if enable_logging {
                self.log_str(&format!("NR78({device_addr:02X}) added"));
            }
            self.nr78_dict.insert(device_addr, 0);
        }
    }

    fn nr78_dict_remove(&mut self, device_addr: u8, enable_logging: bool) {
        if self.nr78_dict.remove(&device_addr).is_some() && enable_logging {
            self.log_str(&format!("NR78({device_addr:02X}) removed"));
        }
    }

    /// Regeneration time wait, send, echo check: the common send part of
    /// the K-line handlers. `send_buf` holds the telegram with checksum.
    fn send_with_echo(&mut self, send_buf: &[u8], rec: &mut [u8], enable_logging: bool) -> IfResult<()> {
        if enable_logging {
            self.log_data(send_buf, "Send");
        }
        while self.within_ms(self.last_response_tick, self.par_regen_time) {
            self.sleep_ms(1);
        }
        self.last_comm_tick = self.now();
        if !self.send_data_ibt(send_buf, self.par_send_set_dtr, self.par_interbyte_time) {
            if enable_logging {
                self.log_str("*** Sending failed");
            }
            return Err(ErrorCode::EDIABAS_IFH_0003);
        }
        if self.has_adapter_echo {
            // remove remote echo
            let send_length = send_buf.len();
            if !self.receive_data(rec, 0, send_length, ECHO_TIMEOUT, self.par_timeout_tel_end, false) {
                if enable_logging {
                    self.log_str("*** No echo received");
                }
                self.flush_receive(rec, ECHO_TIMEOUT, enable_logging);
                return Err(ErrorCode::EDIABAS_IFH_0003);
            }
            if enable_logging {
                self.log_data(&rec[..send_length], "Echo");
            }
            if rec[..send_length] != *send_buf {
                if enable_logging {
                    self.log_str("*** Echo incorrect");
                }
                self.flush_receive(rec, self.par_timeout_std, enable_logging);
                return Err(ErrorCode::EDIABAS_IFH_0003);
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // KWP2000 telegram layer (BMW-FAST, KWP2000 BMW, ISO KWP2000)
    // -----------------------------------------------------------------------

    /// C#: `TransKwp2000(sendData, ..., enableLogging)`: one BMW-FAST
    /// telegram (format byte, target, source, [length], data, sum checksum).
    /// `send` empty: only receive. NR78 extends the timeout, NR21/NR23
    /// repeat the request (if configured). `rec_buf_len` is the size of the
    /// C# receive buffer. Returns the response including checksum.
    pub fn trans_kwp2000_tel(&mut self, send: &[u8], rec_buf_len: usize, enable_logging: bool) -> IfResult<Vec<u8>> {
        let mut nr_send_count = 0;
        'restart: loop {
            let mut rec = vec![0u8; rec_buf_len];

            if !send.is_empty() {
                self.nr78_dict.clear();
                // C# works on the 0x800 byte send buffer.
                let mut send_buf = vec![0u8; TRANS_BUFFER_SIZE.max(send.len())];
                send_buf[..send.len()].copy_from_slice(send);
                let mut send_length = tel_length_bmw_fast(&send_buf).unwrap_or(0);
                if send_length >= send_buf.len() {
                    send_buf.resize(send_length + 1, 0);
                }
                if !self.par_checksum_by_user {
                    send_buf[send_length] = calc_checksum_bmw_fast(&send_buf, send_length);
                }
                send_length += 1;
                self.send_with_echo(&send_buf[..send_length], &mut rec, enable_logging)?;
            }

            loop {
                let timeout = if !self.nr78_dict.is_empty() { self.par_timeout_nr78 } else { self.par_timeout_std };
                let tel_end = self.par_timeout_tel_end;
                // header byte
                let mut head_length = 4;
                if !self.receive_data(&mut rec, 0, head_length, timeout, tel_end, false) {
                    if enable_logging {
                        self.log_str("*** No header received");
                    }
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }

                if (rec[0] & 0xC0) != 0x80 {
                    if enable_logging {
                        self.log_data(&rec[..4], "Head");
                        self.log_str("*** Invalid header");
                    }
                    self.flush_receive(&mut rec, timeout, enable_logging);
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }

                if (rec[0] & 0x3F) == 0x00 && rec[3] == 0 {
                    // 2 byte length
                    if !self.receive_data(&mut rec, head_length, 2, timeout, tel_end, false) {
                        if enable_logging {
                            self.log_str("*** No length received");
                        }
                        return Err(ErrorCode::EDIABAS_IFH_0009);
                    }
                    head_length += 2;
                }

                let rec_length = tel_length_bmw_fast(&rec).unwrap_or(0);
                if !self.receive_data(&mut rec, head_length, (rec_length + 1).saturating_sub(head_length), tel_end, tel_end, false) {
                    if enable_logging {
                        self.log_data(&rec[..4], "*** Rec");
                        self.log_str(&format!("*** No tail received: Length={rec_length}"));
                    }
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }
                if enable_logging {
                    self.log_data(&rec[..rec_length + 1], "Resp");
                }
                if !self.par_checksum_no_check && calc_checksum_bmw_fast(&rec, rec_length) != rec[rec_length] {
                    if enable_logging {
                        self.log_str("*** Checksum incorrect");
                    }
                    self.flush_receive(&mut rec, timeout, enable_logging);
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }

                let mut data_len = (rec[0] & 0x3F) as usize;
                let mut data_start = 3;
                if data_len == 0 {
                    // with length byte
                    data_len = rec[3] as usize;
                    data_start += 1;
                }
                if data_len == 3 && rec[data_start] == 0x7F {
                    let mut nr_request_time = 0;
                    let mut nr_retries = 0;
                    if rec[data_start + 2] == 0x21 {
                        nr_request_time = self.par_request_time_nr21;
                        nr_retries = self.par_retry_nr21;
                    }
                    if rec[data_start + 2] == 0x23 {
                        nr_request_time = self.par_request_time_nr23;
                        nr_retries = self.par_retry_nr23;
                    }
                    if nr_request_time > 0 && nr_retries > 0 {
                        if nr_send_count >= nr_retries {
                            if enable_logging {
                                self.log_str("*** NR21/23 exceeded");
                            }
                            break;
                        }
                        if enable_logging {
                            self.log_str("NR21/23 request");
                        }
                        self.sleep_ms(nr_request_time);
                        nr_send_count += 1;
                        continue 'restart;
                    }
                }

                if data_len == 3 && rec[data_start] == 0x7F && rec[data_start + 2] == 0x78 {
                    // negative response 0x78
                    self.nr78_dict_add(rec[2], enable_logging);
                } else {
                    self.nr78_dict_remove(rec[2], enable_logging);
                    break;
                }
                if self.nr78_dict.is_empty() {
                    break;
                }
            }

            self.last_response_tick = self.now();
            let len = tel_length_bmw_fast(&rec).unwrap_or(0) + 1;
            rec.truncate(len);
            return Ok(rec);
        }
    }

    // -----------------------------------------------------------------------
    // KWP2000 BMW (concept 0x010C)
    // -----------------------------------------------------------------------

    /// C#: `TransKwp2000Bmw`: fast init + start communication when the ECU is
    /// not connected, then the BMW-FAST telegram.
    pub fn trans_kwp2000_bmw(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        if !self.ecu_connected {
            while self.within_ms(self.last_comm_tick, self.par_timeout_std) {
                self.sleep_ms(10);
            }
            if !self.par_start_comm_tel.is_empty() {
                if !self.send_wake_fast_init(self.par_send_set_dtr) {
                    self.log_str("*** Sending wake fast init failed");
                    return Err(ErrorCode::EDIABAS_IFH_0003);
                }
                self.last_comm_tick = self.now();
                let start_comm = self.par_start_comm_tel.clone();
                if let Err(e) = self.trans_kwp2000_tel(&start_comm, TRANS_BUFFER_SIZE, true) {
                    self.log_str("*** Sending start communication failed");
                    return Err(e);
                }
            }
            self.last_comm_tick = self.now();
            self.ecu_connected = true;
        }
        match self.trans_kwp2000_tel(send, TRANS_BUFFER_SIZE, true) {
            Ok(r) => {
                self.last_comm_tick = self.now();
                Ok(r)
            }
            Err(e) => {
                self.ecu_connected = false;
                Err(e)
            }
        }
    }

    /// C#: `IdleKwp2000Bmw`: tester present after `par_tester_present_time`.
    pub fn idle_kwp2000_bmw(&mut self) -> IfResult<()> {
        if !self.ecu_connected {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        if self.within_ms(self.last_comm_tick, self.par_tester_present_time) {
            return Ok(());
        }
        if !self.par_tester_present_tel.is_empty() {
            let tel = self.par_tester_present_tel.clone();
            if let Err(e) = self.trans_kwp2000_tel(&tel, KWP1281_BUFFER_SIZE, false) {
                self.ecu_connected = false;
                return Err(e);
            }
        }
        self.last_comm_tick = self.now();
        Ok(())
    }

    /// C#: `FrequentKwp2000Bmw`.
    pub fn frequent_kwp2000_bmw(&mut self) -> IfResult<()> {
        if self.send_buffer_frequent.is_empty() {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        let send = self.send_buffer_frequent.clone();
        let r = self.trans_kwp2000_bmw(&send);
        self.store_frequent(r)
    }

    fn store_frequent(&mut self, r: IfResult<Vec<u8>>) -> IfResult<()> {
        match r {
            Ok(v) => {
                self.rec_buffer_frequent = v;
                Ok(())
            }
            Err(e) => {
                self.rec_buffer_frequent.clear();
                Err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // KWP2000* (concept 0x010D)
    // -----------------------------------------------------------------------

    /// C#: `TransKwp2000S`: header (format, target, source, length), XOR checksum.
    pub fn trans_kwp2000s(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        let mut rec = vec![0u8; TRANS_BUFFER_SIZE];
        if !send.is_empty() {
            self.nr78_dict.clear();
            let mut send_buf = vec![0u8; TRANS_BUFFER_SIZE.max(send.len())];
            send_buf[..send.len()].copy_from_slice(send);
            let mut send_length = tel_length_kwp2000s(&send_buf);
            if send_length >= send_buf.len() {
                send_buf.resize(send_length + 1, 0);
            }
            if !self.par_checksum_by_user {
                send_buf[send_length] = calc_checksum_xor(&send_buf, send_length);
            }
            send_length += 1;
            self.send_with_echo(&send_buf[..send_length], &mut rec, true)?;
        }

        loop {
            let timeout = if !self.nr78_dict.is_empty() { self.par_timeout_nr78 } else { self.par_timeout_std };
            let tel_end = self.par_timeout_tel_end;
            // header byte
            if !self.receive_data(&mut rec, 0, 4, timeout, tel_end, false) {
                self.log_str("*** No header received");
                return Err(ErrorCode::EDIABAS_IFH_0009);
            }
            let rec_length = tel_length_kwp2000s(&rec);
            if !self.receive_data(&mut rec, 4, rec_length - 3, tel_end, tel_end, false) {
                self.log_data(&rec[..4], "*** Rec");
                self.log_str(&format!("*** No tail received: Length={rec_length}"));
                return Err(ErrorCode::EDIABAS_IFH_0009);
            }
            self.log_data(&rec[..rec_length + 1], "Resp");
            if !self.par_checksum_no_check && calc_checksum_xor(&rec, rec_length) != rec[rec_length] {
                self.log_str("*** Checksum incorrect");
                self.flush_receive(&mut rec, timeout, true);
                return Err(ErrorCode::EDIABAS_IFH_0009);
            }
            let data_len = rec[3];
            let data_start = 4;
            if data_len == 3 && rec[data_start] == 0x7F && rec[data_start + 2] == 0x78 {
                // negative response 0x78
                self.nr78_dict_add(rec[2], true);
            } else {
                self.nr78_dict_remove(rec[2], true);
                break;
            }
            if self.nr78_dict.is_empty() {
                break;
            }
        }

        self.last_response_tick = self.now();
        let len = tel_length_kwp2000s(&rec) + 1;
        rec.truncate(len);
        Ok(rec)
    }

    // -----------------------------------------------------------------------
    // DS2 (concepts 1, 5, 6)
    // -----------------------------------------------------------------------

    /// C#: `TransDs2`: request + XOR checksum; response length from
    /// `comm_answer_len`.
    pub fn trans_ds2(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        let mut rec = vec![0u8; TRANS_BUFFER_SIZE];
        if !send.is_empty() {
            let mut send_buf = send.to_vec();
            if !self.par_checksum_by_user {
                send_buf.push(calc_checksum_xor(send, send.len()));
            }
            self.send_with_echo(&send_buf, &mut rec, true)?;
        }

        // header byte
        let mut header_len = self.comm_answer_len[0] as i32;
        if header_len < 0 {
            header_len = -header_len + 1;
        }
        if header_len == 0 {
            self.log_str("*** Header length zero");
            return Err(ErrorCode::EDIABAS_IFH_0041);
        }
        let header_len = header_len as usize;
        if !self.receive_data(&mut rec, 0, header_len, self.par_timeout_std, self.par_timeout_tel_end, false) {
            self.log_str("*** No header received");
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }

        let rec_length = self.tel_length_ds2(&rec);
        if rec_length <= 0 {
            // C#: only 0 is checked, a negative length throws later
            self.log_str("*** Receive length zero");
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        let rec_length = rec_length as usize;
        let tel_end = self.par_timeout_tel_end;
        if rec_length > header_len && !self.receive_data(&mut rec, header_len, rec_length - header_len, tel_end, tel_end, false) {
            self.log_data(&rec[..4], "*** Rec");
            self.log_str(&format!("*** No tail received: Length={rec_length}"));
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        if rec_length > rec.len() {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        self.log_data(&rec[..rec_length], "Resp");
        if !self.par_checksum_no_check && calc_checksum_xor(&rec, rec_length - 1) != rec[rec_length - 1] {
            self.log_str("*** Checksum incorrect");
            self.flush_receive(&mut rec, self.par_timeout_std, true);
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }

        self.last_response_tick = self.now();
        rec.truncate(rec_length);
        Ok(rec)
    }

    /// C#: `FrequentDs2`.
    pub fn frequent_ds2(&mut self) -> IfResult<()> {
        if self.send_buffer_frequent.is_empty() {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        let send = self.send_buffer_frequent.clone();
        let r = self.trans_ds2(&send);
        self.store_frequent(r)
    }

    /// C#: `TelLengthDs2`: telegram length with checksum.
    fn tel_length_ds2(&self, data: &[u8]) -> i32 {
        let mut tel_length = self.comm_answer_len[0] as i32; // >0 fix length
        if tel_length < 0 {
            // offset in buffer
            let offset = (-tel_length) as usize;
            if data.len() < offset {
                return 0;
            }
            tel_length = data[offset] as i32 + self.comm_answer_len[1] as i32; // + answer offset
        }
        tel_length
    }

    // -----------------------------------------------------------------------
    // ISO KWP2000 with 5 baud init (C#: EDIC concept 0 / 0x81)
    // -----------------------------------------------------------------------

    /// C#: `TransKwp2000`: 5 baud wake up when the ECU is not connected,
    /// then the ISO telegram with header built from the EDIC addresses.
    /// KWP1281 ECUs are not supported (EDIABAS_IFH_0014).
    pub fn trans_kwp2000_iso(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        if !self.ecu_connected {
            self.kwp_mode = KwpMode::Undefined;
            self.key_bytes.clear();
            let mut key_bytes_list: Vec<u8> = Vec::new();
            let delay_time = self.par_edic_init_delay;
            self.par_edic_init_delay = KWP1281_INIT_LONG_DELAY;

            self.log_str(&format!("Delay time: {delay_time}"));
            while self.within_ms(self.last_comm_tick, delay_time) {
                self.sleep_ms(10);
            }

            self.log_str("Establish connection");
            let data_bits = self.current_data_bits;
            let r = self.link.as_mut().map(|l| l.set_config(9600, data_bits, Parity::None));
            if !matches!(r, Some(Ok(()))) {
                self.log_str("*** Set baud rate failed");
                return Err(ErrorCode::EDIABAS_IFH_0041);
            }
            self.current_baud_rate = 9600;
            self.current_parity = Parity::None;

            if !self.set_dtr_signal(self.par_send_set_dtr) {
                self.log_str("*** Set DTR failed");
                return Err(ErrorCode::EDIABAS_IFH_0041);
            }

            let mut buf = [0u8; KWP1281_BUFFER_SIZE];
            self.last_comm_tick = self.now();
            if !self.send_wake_address_5baud(self.par_edic_wake_address, self.par_send_set_dtr, true, self.par_edic_w4a) {
                self.last_comm_tick = self.now();
                self.log_str("*** Sending wake address failed");
                return Err(ErrorCode::EDIABAS_IFH_0003);
            }

            self.last_comm_tick = self.now();
            if self.has_auto_baud_rate {
                if !self.receive_data(&mut buf, 0, 2, self.par_edic_w1, self.par_edic_w1, false) {
                    self.log_str("*** No wake response");
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }
                let baud_rate = (((buf[0] as u32) << 8) + buf[1] as u32) << 1;
                if baud_rate == 0 {
                    self.log_str("*** Invalid baud rate");
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }
                self.log_str(&format!("Baud rate {baud_rate} detected"));
                self.current_baud_rate = baud_rate;
                self.current_parity = Parity::None;
            } else {
                if !self.receive_data(&mut buf, 0, 1, self.par_edic_w1, self.par_edic_w1, false) {
                    self.log_str("*** No wake response");
                    return Err(ErrorCode::EDIABAS_IFH_0009);
                }
                self.log_str(&format!("Baud rate byte: {:02X}", buf[0]));
                if buf[0] == 0x55 {
                    self.log_str("Baud rate 9.6k detected");
                } else {
                    // baud rate different
                    if (buf[0] & 0x87) != 0x85 {
                        self.log_str("*** Invalid baud rate");
                        return Err(ErrorCode::EDIABAS_IFH_0009);
                    }
                    self.log_str("Baud rate 10.4k detected");
                    let r = self.link.as_mut().map(|l| l.set_config(10400, data_bits, Parity::None));
                    if !matches!(r, Some(Ok(()))) {
                        self.log_str("*** Set baud rate failed");
                        return Err(ErrorCode::EDIABAS_IFH_0041);
                    }
                }
                self.current_baud_rate = 10400;
                self.current_parity = Parity::None;
            }

            self.last_comm_tick = self.now();
            if !self.receive_data(&mut buf, 0, 2, self.par_edic_w2, self.par_edic_w3, false) {
                self.log_str("*** No key bytes received");
                return Err(ErrorCode::EDIABAS_IFH_0009);
            }

            self.last_comm_tick = self.now();
            self.log_str(&format!("Key bytes: {:02X} {:02X}", buf[0], buf[1]));
            self.kwp_mode = if buf[1] == 0x8F { KwpMode::Kwp2000 } else { KwpMode::Kwp1281 };

            key_bytes_list.push(buf[0]);
            key_bytes_list.push(buf[1]);
            key_bytes_list.push(if self.kwp_mode == KwpMode::Kwp2000 { !self.par_edic_wake_address } else { 0x00 });
            key_bytes_list.push(self.current_baud_rate as u8);
            key_bytes_list.push((self.current_baud_rate >> 8) as u8);

            self.sleep_ms(self.par_edic_w4a);
            if !self.has_auto_baud_rate {
                let response = [!buf[1]];
                if !self.send_data(&response, self.par_send_set_dtr, false) {
                    self.ecu_connected = false;
                    self.log_str("*** Sending key byte response failed");
                    return Err(ErrorCode::EDIABAS_IFH_0003);
                }
            }
            self.last_comm_tick = self.now();

            let sgbd_name = self.sgbd_file_name.clone();
            let prg_file = sgbd_name.to_ascii_lowercase().ends_with(".prg");
            let mut protocol_mismatch = false;
            let r = match self.kwp_mode {
                KwpMode::Kwp2000 => {
                    if prg_file && sgbd_name.contains("1281") {
                        protocol_mismatch = true;
                    }
                    self.init_kwp2000(key_bytes_list)
                }
                KwpMode::Kwp1281 => {
                    self.log_str("*** KWP1281 not implemented");
                    Err(ErrorCode::EDIABAS_IFH_0014)
                }
                KwpMode::Undefined => Err(ErrorCode::EDIABAS_IFH_0014),
            };
            r?;

            self.valid_kline_response = true;
            self.log_str("Valid K-LINE response has been detected");

            if protocol_mismatch {
                self.log_str("*** SGBD protocol mismatch");
                self.ecu_connected = false;
                self.par_edic_add_retries = 3;
                self.par_edic_init_delay = KWP1281_INIT_SHORT_DELAY;
                return Err(ErrorCode::EDIABAS_IFH_0010); // no response
            }
        }
        match self.kwp_mode {
            KwpMode::Kwp2000 => {
                if !send.is_empty() {
                    // create full telegram
                    let mut buf = Vec::with_capacity(send.len() + 5);
                    if send.len() > 0x3F {
                        buf.extend_from_slice(&[0x80, self.par_edic_ecu_address, self.par_edic_tester_address, send.len() as u8]);
                    } else {
                        buf.extend_from_slice(&[0x80 | send.len() as u8, self.par_edic_ecu_address, self.par_edic_tester_address]);
                    }
                    buf.extend_from_slice(send);
                    buf.push(0); // checksum
                    return self.process_kwp2000(&buf);
                }
                self.process_kwp2000(send)
            }
            _ => Err(ErrorCode::EDIABAS_IFH_0014), // concept not implemented
        }
    }

    /// C#: `InitKwp2000`: inverted wake address from the ECU.
    fn init_kwp2000(&mut self, key_bytes_list: Vec<u8>) -> IfResult<()> {
        let mut buf = [0u8; KWP1281_BUFFER_SIZE];
        if !self.receive_data(&mut buf, 0, 1, self.par_edic_w4, self.par_edic_w4, false) {
            self.log_str("*** No wake address received");
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        self.log_str(&format!("Wake address byte: {:02X}", buf[0]));
        if self.par_edic_wake_address != !buf[0] {
            self.log_str("*** Invalid wake address received");
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        self.ecu_connected = true;
        self.key_bytes = key_bytes_list;
        let kb = self.key_bytes.clone();
        self.log_data(&kb, "ID bytes");
        Ok(())
    }

    /// C#: `ProcessKwp2000`: request, then collects further telegrams until
    /// the receive times out.
    fn process_kwp2000(&mut self, send: &[u8]) -> IfResult<Vec<u8>> {
        if send.is_empty() {
            return Ok(Vec::new());
        }
        let mut rec = self.trans_kwp2000_tel(send, TRANS_BUFFER_SIZE, true)?;
        loop {
            match self.trans_kwp2000_tel(&[], TRANS_BUFFER_SIZE, true) {
                Err(ErrorCode::EDIABAS_IFH_0003) => return Err(ErrorCode::EDIABAS_IFH_0003),
                Err(_) => return Ok(rec),
                Ok(t) => {
                    if rec.len() + t.len() <= TRANS_BUFFER_SIZE {
                        rec.extend_from_slice(&t);
                    }
                }
            }
        }
    }

    /// C#: `IdleKwp2000`.
    pub fn idle_kwp2000(&mut self) -> IfResult<()> {
        if !self.ecu_connected {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        match self.kwp_mode {
            KwpMode::Kwp2000 => {
                if self.within_ms(self.last_comm_tick, self.par_edic_tester_present_time) {
                    return Ok(());
                }
                if !self.par_edic_tester_present_tel.is_empty() {
                    let tel = self.par_edic_tester_present_tel.clone();
                    if let Err(e) = self.trans_kwp2000_tel(&tel, KWP1281_BUFFER_SIZE, false) {
                        self.ecu_connected = false;
                        return Err(e);
                    }
                }
                self.last_comm_tick = self.now();
                Ok(())
            }
            // KWP1281 not ported
            _ => Err(ErrorCode::EDIABAS_IFH_0014),
        }
    }

    /// C#: `FrequentKwp2000`.
    pub fn frequent_kwp2000(&mut self) -> IfResult<()> {
        if !self.ecu_connected {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        if self.send_buffer_frequent.is_empty() {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        match self.kwp_mode {
            KwpMode::Kwp2000 => {
                let send = self.send_buffer_frequent.clone();
                let r = self.trans_kwp2000_tel(&send, TRANS_BUFFER_SIZE, false);
                let r = self.store_frequent(r);
                if r.is_err() {
                    self.ecu_connected = false;
                }
                r
            }
            _ => Err(ErrorCode::EDIABAS_IFH_0014),
        }
    }

    /// C#: `FinishKwp2000`: stop communication (0x82).
    pub fn finish_kwp2000(&mut self) -> IfResult<()> {
        if !self.ecu_connected {
            return Err(ErrorCode::EDIABAS_IFH_0009);
        }
        match self.kwp_mode {
            KwpMode::Kwp2000 => {
                let finish_tel = [0x81, self.par_edic_ecu_address, self.par_edic_tester_address, 0x82, 0x00];
                let r = self.trans_kwp2000_tel(&finish_tel[..4], KWP1281_BUFFER_SIZE, false);
                self.ecu_connected = false;
                r.map(|_| ())
            }
            _ => Err(ErrorCode::EDIABAS_IFH_0014),
        }
    }
}
