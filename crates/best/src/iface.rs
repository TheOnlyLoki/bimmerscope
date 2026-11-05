//! Diagnostic interface used by the `x*` opcodes (C#: `EdInterfaceBase`).
//! Errors are returned as `ErrorCode`; the opcode passes them to
//! `Vm::set_error`, like the C# interfaces call `SetError` themselves.

use crate::ErrorCode;

pub type IfResult<T> = Result<T, ErrorCode>;

pub trait Interface {
    /// C#: `InterfaceType`, e.g. "OBD".
    fn interface_type(&self) -> &str;
    fn interface_version(&self) -> u32;
    fn connected(&self) -> bool;

    fn interface_connect(&mut self) -> IfResult<()>;
    fn interface_disconnect(&mut self) -> IfResult<()>;
    fn interface_reset(&mut self) -> IfResult<()>;
    fn interface_boot(&mut self) -> IfResult<()>;

    /// Sends one request telegram and returns the response.
    fn transmit_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>>;
    fn transmit_frequent(&mut self, request: &[u8]) -> IfResult<()>;
    fn receive_frequent(&mut self) -> IfResult<Vec<u8>>;
    fn stop_frequent(&mut self) -> IfResult<()>;
    fn raw_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>>;

    /// C#: `CommParameter` (set via `xsetpar`).
    fn comm_parameter(&self) -> &[u32];
    fn set_comm_parameter(&mut self, pars: Vec<u32>) -> IfResult<()>;
    /// C#: `CommAnswerLen` (set via `xawlen`).
    fn comm_answer_len(&self) -> &[i16];
    fn set_comm_answer_len(&mut self, len: Vec<i16>) -> IfResult<()>;
    fn comm_repeats(&self) -> u32;
    fn set_comm_repeats(&mut self, repeats: u32);

    fn key_bytes(&mut self) -> IfResult<Vec<u8>>;
    fn state(&mut self) -> IfResult<Vec<u8>>;
    fn loop_test(&mut self) -> IfResult<u32>;
    /// Millivolts; negative if unknown.
    fn battery_voltage(&mut self) -> IfResult<i64>;
    fn ignition_voltage(&mut self) -> IfResult<i64>;

    fn get_port(&mut self, index: u32) -> IfResult<i64>;
    fn set_port(&mut self, index: u32, value: u32) -> IfResult<()>;
    fn set_program_voltage(&mut self, voltage: u32) -> IfResult<()>;
    fn switch_si_relais(&mut self, time: u32) -> IfResult<()>;
}
