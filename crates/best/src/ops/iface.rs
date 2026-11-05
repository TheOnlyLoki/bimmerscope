//! Opcodes: xconnect, xhangup, xsetpar, xawlen, xsend, xsendf, xrequf, xstopf, xkeyb, xstate, xboot, xreset, xtype, xvers, xreps, xbatt, xgetport, xignit, xloopt, xprog, xraw, xsetport, xsireset.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables)]

use super::OpCode;
use crate::iface::IfResult;
use crate::operand::{Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `interfaceClass == null || !interfaceClass.Connected`
/// (the caller raises IFH_0056 when this is false).
fn is_connected(vm: &Vm) -> bool {
    vm.iface.as_ref().is_some_and(|i| i.connected())
}

/// The interface after the caller's presence check.
fn iface_mut(vm: &mut Vm) -> VmResult<&mut dyn crate::Interface> {
    match vm.iface.as_deref_mut() {
        Some(i) => Ok(i),
        None => internal("interface not present"),
    }
}

/// C# `if (interface.Method(...))` / void call: on failure the interface
/// already called `SetError` in C#, so the error is raised here.
/// Returns `None` if the error was trapped (C#: call returned false).
fn ok_or_set<T>(vm: &mut Vm, r: IfResult<T>) -> VmResult<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) => {
            vm.set_error(e)?;
            Ok(None)
        }
    }
}

/// C#: `OpXconnect` (xconnect)
pub(crate) fn op_xconnect(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return internal("OpXconnect: No communication class present");
    }
    let r = iface_mut(vm)?.interface_connect();
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXhangup` (xhangup)
pub(crate) fn op_xhangup(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return internal("OpXhangup: No communication class present");
    }
    let r = iface_mut(vm)?.interface_disconnect();
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXsetpar` (xsetpar)
pub(crate) fn op_xsetpar(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let data = vm.op_array(arg0)?;
    let pars = parse_comm_parameter(&data);
    let r = iface_mut(vm)?.set_comm_parameter(pars);
    ok_or_set(vm, r)?;
    Ok(())
}

/// Element list of `OpXsetpar`. Byte 1 is the element type: 0x00 = 16 bit,
/// 0x01 = 32 bit, 0xFF = 8 bit (little endian). Any other type, or a length
/// that is not a multiple of the element size, gives an empty list.
fn parse_comm_parameter(data: &[u8]) -> Vec<u32> {
    let data_type_len = if data.len() >= 2 {
        match data[1] {
            0x00 => 2,
            0x01 => 4,
            0xFF => 1,
            _ => 0,
        }
    } else {
        0
    };
    if data_type_len == 0 || data.len() % data_type_len != 0 {
        return Vec::new();
    }
    data.chunks_exact(data_type_len).map(|c| c.iter().rev().fold(0u32, |acc, &b| acc << 8 | b as u32)).collect()
}

/// C#: `OpXawlen` (xawlen)
pub(crate) fn op_xawlen(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let data = vm.op_array(arg0)?;
    if data.len() & 0x01 != 0 {
        return internal("OpXawlen: Invalid data length");
    }
    let answer: Vec<i16> = data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    let r = iface_mut(vm)?.set_comm_answer_len(answer);
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXbatt` (xbatt)
pub(crate) fn op_xbatt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXbat: Invalid type");
    }
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.battery_voltage();
    if let Some(voltage) = ok_or_set(vm, r)? {
        // Int64.MinValue means "unknown": the register is left unchanged.
        if voltage != i64::MIN {
            vm.op_set_raw(arg0, Value::Int(voltage as u32), 1)?;
        }
    }
    Ok(())
}

/// C#: `OpXboot` (xboot)
pub(crate) fn op_xboot(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.interface_boot();
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXgetport` (xgetport)
pub(crate) fn op_xgetport(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXgetport: Invalid type");
    }
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let port_index = vm.op_value(arg1, 0)? & 0xFF;
    let r = iface_mut(vm)?.get_port(port_index);
    if let Some(port_value) = ok_or_set(vm, r)? {
        // C# `(EdValueType)Int64`: truncates to 32 bit.
        vm.op_set_raw(arg0, Value::Int(port_value as u32), 1)?;
    }
    Ok(())
}

/// C#: `OpXignit` (xignit)
pub(crate) fn op_xignit(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXignit: Invalid type");
    }
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.ignition_voltage();
    if let Some(voltage) = ok_or_set(vm, r)? {
        // Int64.MinValue means "unknown": the register is left unchanged.
        if voltage != i64::MIN {
            vm.op_set_raw(arg0, Value::Int(voltage as u32), 1)?;
        }
    }
    Ok(())
}

/// C#: `OpXkeyb` (xkeyb)
pub(crate) fn op_xkeyb(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXkeyb: Invalid type");
    }
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    // C# skips the write when KeyBytes is null; the Rust trait has no null,
    // so a successful read is always written.
    let r = iface_mut(vm)?.key_bytes();
    if let Some(key) = ok_or_set(vm, r)? {
        vm.op_set_raw(arg0, Value::Bytes(key), 1)?;
    }
    Ok(())
}

/// C#: `OpXloopt` (xloopt)
pub(crate) fn op_xloopt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXloopt: Invalid type");
    }
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.loop_test();
    if let Some(test_result) = ok_or_set(vm, r)? {
        vm.op_set_raw(arg0, Value::Int(test_result), 1)?;
    }
    Ok(())
}

/// C#: `OpXprog` (xprog)
pub(crate) fn op_xprog(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let voltage = vm.op_value(arg0, 0)?;
    let r = iface_mut(vm)?.set_program_voltage(voltage);
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXraw` (xraw)
pub(crate) fn op_xraw(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXraw: Invalid type");
    }
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let request = vm.op_array(arg1)?;
    let r = iface_mut(vm)?.raw_data(&request);
    if let Some(response) = ok_or_set(vm, r)? {
        vm.op_set_raw(arg0, Value::Bytes(response), 1)?;
    }
    Ok(())
}

/// C#: `OpXreps` (xreps)
pub(crate) fn op_xreps(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let repeats = vm.op_value(arg0, 0)?;
    iface_mut(vm)?.set_comm_repeats(repeats);
    Ok(())
}

/// C#: `OpXrequf` (xrequf)
pub(crate) fn op_xrequf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXrequf: Invalid type");
    }
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.receive_frequent();
    if let Some(response) = ok_or_set(vm, r)? {
        vm.op_set_raw(arg0, Value::Bytes(response), 1)?;
    }
    Ok(())
}

/// C#: `OpXreset` (xreset)
pub(crate) fn op_xreset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.interface_reset();
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXsend` (xsend)
pub(crate) fn op_xsend(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXsend: Invalid type");
    }
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let request = vm.op_array(arg1)?;
    let r = iface_mut(vm)?.transmit_data(&request);
    if let Some(response) = ok_or_set(vm, r)? {
        vm.op_set_raw(arg0, Value::Bytes(response), 1)?;
    }
    Ok(())
}

/// C#: `OpXsendf` (xsendf). C# reads the request from arg0, not arg1.
pub(crate) fn op_xsendf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let request = vm.op_array(arg0)?;
    let r = iface_mut(vm)?.transmit_frequent(&request);
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXsetport` (xsetport)
pub(crate) fn op_xsetport(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let port_data = vm.op_array(arg0)?;
    let port_index = port_data.first().copied().unwrap_or(0) as u32;
    let port_value = vm.op_value(arg1, 0)?;
    let r = iface_mut(vm)?.set_port(port_index, port_value);
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXsireset` (xsireset)
pub(crate) fn op_xsireset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let time_value = vm.op_value(arg0, 0)?;
    let r = iface_mut(vm)?.switch_si_relais(time_value);
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXstate` (xstate)
pub(crate) fn op_xstate(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXstate: Invalid type");
    }
    if vm.iface.is_none() {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.state();
    if let Some(state) = ok_or_set(vm, r)? {
        vm.op_set_raw(arg0, Value::Bytes(state), 1)?;
    }
    Ok(())
}

/// C#: `OpXstopf` (xstopf)
pub(crate) fn op_xstopf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let r = iface_mut(vm)?.stop_frequent();
    ok_or_set(vm, r)?;
    Ok(())
}

/// C#: `OpXtype` (xtype)
pub(crate) fn op_xtype(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXtype: Invalid type");
    }
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let interface_type = iface_mut(vm)?.interface_type().to_string();
    vm.op_set_string(arg0, &interface_type)?;
    Ok(())
}

/// C#: `OpXvers` (xvers)
pub(crate) fn op_xvers(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXvers: Invalid type");
    }
    if !is_connected(vm) {
        return vm.set_error(ErrorCode::EDIABAS_IFH_0056);
    }
    let version = iface_mut(vm)?.interface_version();
    vm.op_set_raw(arg0, Value::Int(version), 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::VmError;
    use crate::regs::Reg;

    type Log = Rc<RefCell<Vec<String>>>;

    /// Interface double. `log` is shared with the test, because the VM
    /// owns the boxed interface.
    #[derive(Default)]
    struct Mock {
        connected: bool,
        response: Vec<u8>,
        pars: Vec<u32>,
        awlen: Vec<i16>,
        repeats: u32,
        /// Error returned by the transmit calls.
        fail: Option<ErrorCode>,
        battery: i64,
        ignition: i64,
        port: i64,
        version: u32,
        loop_result: u32,
        log: Option<Log>,
    }

    impl Mock {
        fn note(&self, s: String) {
            if let Some(log) = &self.log {
                log.borrow_mut().push(s);
            }
        }
    }

    impl crate::Interface for Mock {
        fn interface_type(&self) -> &str {
            "OBD"
        }
        fn interface_version(&self) -> u32 {
            self.version
        }
        fn connected(&self) -> bool {
            self.connected
        }
        fn interface_connect(&mut self) -> IfResult<()> {
            self.note("connect".into());
            self.connected = true;
            Ok(())
        }
        fn interface_disconnect(&mut self) -> IfResult<()> {
            self.note("disconnect".into());
            self.connected = false;
            Ok(())
        }
        fn interface_reset(&mut self) -> IfResult<()> {
            self.note("reset".into());
            Ok(())
        }
        fn interface_boot(&mut self) -> IfResult<()> {
            self.note("boot".into());
            Ok(())
        }
        fn transmit_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>> {
            self.note(format!("send {request:02X?}"));
            match self.fail {
                Some(e) => Err(e),
                None => Ok(self.response.clone()),
            }
        }
        fn transmit_frequent(&mut self, request: &[u8]) -> IfResult<()> {
            self.note(format!("sendf {request:02X?}"));
            match self.fail {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
        fn receive_frequent(&mut self) -> IfResult<Vec<u8>> {
            Ok(self.response.clone())
        }
        fn stop_frequent(&mut self) -> IfResult<()> {
            self.note("stopf".into());
            Ok(())
        }
        fn raw_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>> {
            self.note(format!("raw {request:02X?}"));
            Ok(self.response.clone())
        }
        fn comm_parameter(&self) -> &[u32] {
            &self.pars
        }
        fn set_comm_parameter(&mut self, pars: Vec<u32>) -> IfResult<()> {
            self.pars = pars;
            Ok(())
        }
        fn comm_answer_len(&self) -> &[i16] {
            &self.awlen
        }
        fn set_comm_answer_len(&mut self, len: Vec<i16>) -> IfResult<()> {
            self.awlen = len;
            Ok(())
        }
        fn comm_repeats(&self) -> u32 {
            self.repeats
        }
        fn set_comm_repeats(&mut self, repeats: u32) {
            self.repeats = repeats;
        }
        fn key_bytes(&mut self) -> IfResult<Vec<u8>> {
            Ok(vec![0x55, 0xAA])
        }
        fn state(&mut self) -> IfResult<Vec<u8>> {
            Ok(vec![0x01, 0x02])
        }
        fn loop_test(&mut self) -> IfResult<u32> {
            Ok(self.loop_result)
        }
        fn battery_voltage(&mut self) -> IfResult<i64> {
            Ok(self.battery)
        }
        fn ignition_voltage(&mut self) -> IfResult<i64> {
            Ok(self.ignition)
        }
        fn get_port(&mut self, index: u32) -> IfResult<i64> {
            self.note(format!("getport {index}"));
            Ok(self.port)
        }
        fn set_port(&mut self, index: u32, value: u32) -> IfResult<()> {
            self.note(format!("setport {index} {value}"));
            Ok(())
        }
        fn set_program_voltage(&mut self, voltage: u32) -> IfResult<()> {
            self.note(format!("prog {voltage}"));
            Ok(())
        }
        fn switch_si_relais(&mut self, time: u32) -> IfResult<()> {
            self.note(format!("sireset {time}"));
            Ok(())
        }
    }

    fn oc() -> &'static OpCode {
        &crate::ops::OPCODES[0]
    }

    /// VM with a mock interface; returns the VM and the call log.
    fn vm_with(mock: Mock) -> (Vm, Log) {
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let mut vm = Vm::for_test();
        vm.iface = Some(Box::new(Mock { log: Some(log.clone()), ..mock }));
        (vm, log)
    }

    fn connected() -> Mock {
        Mock { connected: true, ..Default::default() }
    }

    fn assert_ifh_0056(r: VmResult<()>) {
        assert!(matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_IFH_0056))), "got {r:?}");
    }

    fn s0() -> Operand {
        Operand::r(Reg::S(0))
    }

    fn l0() -> Operand {
        Operand::r(Reg::L(0))
    }

    #[test]
    fn xsend_round_trip() {
        let (mut vm, log) = vm_with(Mock { response: vec![0x12, 0x34], ..connected() });
        let (mut a0, mut a1) = (s0(), Operand::imm_str(&[0x01, 0x02]));
        op_xsend(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["send [01, 02]"]);
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x12, 0x34]);
    }

    #[test]
    fn xsend_interface_error_is_raised_and_leaves_arg0() {
        let (mut vm, _log) = vm_with(Mock { fail: Some(ErrorCode::EDIABAS_IFH_0001), ..connected() });
        let (mut a0, mut a1) = (s0(), Operand::imm_str(&[0x01]));
        let r = op_xsend(&mut vm, oc(), &mut a0, &mut a1);
        assert!(matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_IFH_0001))));
        assert_eq!(vm.op_array(&a0).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn xsend_not_connected_raises_ifh_0056() {
        let (mut vm, log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (s0(), Operand::imm_str(&[0x01]));
        assert_ifh_0056(op_xsend(&mut vm, oc(), &mut a0, &mut a1));
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn xsend_no_interface_raises_ifh_0056() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (s0(), Operand::imm_str(&[0x01]));
        assert_ifh_0056(op_xsend(&mut vm, oc(), &mut a0, &mut a1));
    }

    #[test]
    fn xsend_invalid_arg0_is_internal_error() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm8(1), Operand::imm_str(&[0x01]));
        assert!(matches!(op_xsend(&mut vm, oc(), &mut a0, &mut a1), Err(VmError::Internal(_))));
    }

    #[test]
    fn xsetpar_byte_elements() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x05, 0xFF, 0x07]), Operand::none());
        op_xsetpar(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.iface.as_ref().unwrap().comm_parameter(), &[0x05, 0xFF, 0x07]);
    }

    #[test]
    fn xsetpar_int_elements() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x34, 0x12, 0x78, 0x56, 0x00, 0x00]), Operand::none());
        // Type byte is 0x12 here, so this is an unknown type: empty list.
        op_xsetpar(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.iface.as_ref().unwrap().comm_parameter().is_empty());

        let (mut a0, mut a1) = (Operand::imm_str(&[0x34, 0x00, 0x12, 0x00]), Operand::none());
        op_xsetpar(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.iface.as_ref().unwrap().comm_parameter(), &[0x0034, 0x0012]);
    }

    #[test]
    fn xsetpar_long_elements() {
        let (mut vm, _log) = vm_with(connected());
        // Two 32 bit elements, little endian. Byte 1 is the type 0x01.
        let (mut a0, mut a1) = (Operand::imm_str(&[0x00, 0x01, 0x78, 0x56, 0x34, 0x12, 0xEF, 0xCD]), Operand::none());
        op_xsetpar(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.iface.as_ref().unwrap().comm_parameter(), &[0x5678_0100, 0xCDEF_1234]);
    }

    #[test]
    fn xsetpar_bad_length_gives_empty_list() {
        let (mut vm, _log) = vm_with(connected());
        // 3 bytes of 16 bit elements: not a multiple of 2.
        let (mut a0, mut a1) = (Operand::imm_str(&[0x01, 0x00, 0x02]), Operand::none());
        op_xsetpar(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.iface.as_ref().unwrap().comm_parameter().is_empty());
    }

    #[test]
    fn xsetpar_not_connected_raises_ifh_0056() {
        let (mut vm, _log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x05, 0xFF]), Operand::none());
        assert_ifh_0056(op_xsetpar(&mut vm, oc(), &mut a0, &mut a1));
    }

    #[test]
    fn xawlen_sets_signed_answer_lengths() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x01, 0x00, 0xFF, 0xFF]), Operand::none());
        op_xawlen(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.iface.as_ref().unwrap().comm_answer_len(), &[1, -1]);
    }

    #[test]
    fn xawlen_odd_length_is_internal_error() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x01]), Operand::none());
        assert!(matches!(op_xawlen(&mut vm, oc(), &mut a0, &mut a1), Err(VmError::Internal(_))));
    }

    #[test]
    fn xawlen_not_connected_raises_ifh_0056() {
        let (mut vm, _log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x01, 0x00]), Operand::none());
        assert_ifh_0056(op_xawlen(&mut vm, oc(), &mut a0, &mut a1));
    }

    #[test]
    fn xconnect_calls_connect_and_null_is_internal_error() {
        let (mut vm, log) = vm_with(Mock::default());
        op_xconnect(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(log.borrow().as_slice(), ["connect"]);

        let mut vm = Vm::for_test();
        assert!(matches!(op_xconnect(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()), Err(VmError::Internal(_))));
    }

    #[test]
    fn xhangup_calls_disconnect_and_null_is_internal_error() {
        let (mut vm, log) = vm_with(connected());
        op_xhangup(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(log.borrow().as_slice(), ["disconnect"]);

        let mut vm = Vm::for_test();
        assert!(matches!(op_xhangup(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()), Err(VmError::Internal(_))));
    }

    #[test]
    fn xbatt_stores_voltage_and_skips_unknown() {
        let (mut vm, _log) = vm_with(Mock { battery: 12_345, ..Default::default() });
        let (mut a0, mut a1) = (l0(), Operand::none());
        op_xbatt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_value(&a0, 0).unwrap(), 12_345);

        let (mut vm, _log) = vm_with(Mock { battery: i64::MIN, ..Default::default() });
        let (mut a0, mut a1) = (l0(), Operand::none());
        vm.regs.set_value(Reg::L(0), 7).unwrap();
        op_xbatt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_value(&a0, 0).unwrap(), 7);
    }

    #[test]
    fn xboot_calls_boot() {
        let (mut vm, log) = vm_with(Mock::default());
        op_xboot(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(log.borrow().as_slice(), ["boot"]);
    }

    #[test]
    fn xgetport_reads_port_masked_to_byte() {
        let (mut vm, log) = vm_with(Mock { port: 0x1_0042, ..connected() });
        let (mut a0, mut a1) = (l0(), Operand::imm16(0x1FF));
        op_xgetport(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["getport 255"]);
        assert_eq!(vm.op_value(&a0, 0).unwrap(), 0x1_0042);
    }

    #[test]
    fn xignit_stores_voltage() {
        let (mut vm, _log) = vm_with(Mock { ignition: 13_000, ..Default::default() });
        let (mut a0, mut a1) = (l0(), Operand::none());
        op_xignit(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_value(&a0, 0).unwrap(), 13_000);
    }

    #[test]
    fn xkeyb_stores_key_bytes() {
        let (mut vm, _log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (s0(), Operand::none());
        op_xkeyb(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x55, 0xAA]);
    }

    #[test]
    fn xloopt_stores_loop_result() {
        let (mut vm, _log) = vm_with(Mock { loop_result: 42, ..Default::default() });
        let (mut a0, mut a1) = (l0(), Operand::none());
        op_xloopt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_value(&a0, 0).unwrap(), 42);
    }

    #[test]
    fn xprog_sets_program_voltage() {
        let (mut vm, log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (Operand::imm8(21), Operand::none());
        op_xprog(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["prog 21"]);
    }

    #[test]
    fn xraw_round_trip() {
        let (mut vm, log) = vm_with(Mock { response: vec![0xAA], ..connected() });
        let (mut a0, mut a1) = (s0(), Operand::imm_str(&[0x3E]));
        op_xraw(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["raw [3E]"]);
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0xAA]);
    }

    #[test]
    fn xreps_sets_repeats() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm8(3), Operand::none());
        op_xreps(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.iface.as_ref().unwrap().comm_repeats(), 3);
    }

    #[test]
    fn xrequf_receives_frequent_data() {
        let (mut vm, _log) = vm_with(Mock { response: vec![0x01, 0x02], ..connected() });
        let (mut a0, mut a1) = (s0(), Operand::none());
        op_xrequf(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x01, 0x02]);
    }

    #[test]
    fn xreset_calls_reset() {
        let (mut vm, log) = vm_with(Mock::default());
        op_xreset(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(log.borrow().as_slice(), ["reset"]);
    }

    #[test]
    fn xsendf_sends_arg0_as_request() {
        let (mut vm, log) = vm_with(connected());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x0B, 0x0C]), Operand::none());
        op_xsendf(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["sendf [0B, 0C]"]);
    }

    #[test]
    fn xstopf_calls_stop() {
        let (mut vm, log) = vm_with(connected());
        op_xstopf(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(log.borrow().as_slice(), ["stopf"]);
    }

    #[test]
    fn xstate_stores_state() {
        let (mut vm, _log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (s0(), Operand::none());
        op_xstate(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x01, 0x02]);
    }

    #[test]
    fn xtype_stores_interface_type_as_string() {
        let (mut vm, _log) = vm_with(connected());
        let (mut a0, mut a1) = (s0(), Operand::none());
        op_xtype(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_string(&a0).unwrap(), "OBD");
    }

    #[test]
    fn xvers_stores_version() {
        let (mut vm, _log) = vm_with(Mock { version: 7, ..connected() });
        let (mut a0, mut a1) = (l0(), Operand::none());
        op_xvers(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_value(&a0, 0).unwrap(), 7);
    }

    #[test]
    fn xsetport_passes_index_and_value() {
        let (mut vm, log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (Operand::imm_str(&[0x03]), Operand::imm8(9));
        op_xsetport(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["setport 3 9"]);
    }

    #[test]
    fn xsireset_passes_time() {
        let (mut vm, log) = vm_with(Mock::default());
        let (mut a0, mut a1) = (Operand::imm16(500), Operand::none());
        op_xsireset(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(log.borrow().as_slice(), ["sireset 500"]);
    }
}
