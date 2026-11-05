//! Opcodes: ergb, ergw, ergd, ergi, ergr, ergs, ergy, enewset, etag, eerr, ergc, ergl, ergsysi, generr.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::Operand;
use crate::result::{ResultData, ResultType, ResultValue};
use crate::vm::Vm;
use crate::{ErrorCode, VmResult};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
/// The result handlers need no extra state: result sets live on `Vm`
/// (`results`, `result_sets`, `sys_results`), `request_init` and
/// `requested_results` too.
#[derive(Debug, Default)]
pub struct State {}

/// C#: `_trapBitDict` keys in insertion order (`EdiabasNet.cs`). The C# dict
/// only holds BIP_0011, IFH_0069 and IFH_0074 for newer interface versions;
/// they are included unconditionally, as in `ErrorCode::trap_bit`.
const TRAP_KEYS: [ErrorCode; 23] = [
    ErrorCode::EDIABAS_BIP_0002,
    ErrorCode::EDIABAS_BIP_0006,
    ErrorCode::EDIABAS_BIP_0009,
    ErrorCode::EDIABAS_BIP_0010,
    ErrorCode::EDIABAS_IFH_0001,
    ErrorCode::EDIABAS_IFH_0002,
    ErrorCode::EDIABAS_IFH_0003,
    ErrorCode::EDIABAS_IFH_0004,
    ErrorCode::EDIABAS_IFH_0005,
    ErrorCode::EDIABAS_IFH_0006,
    ErrorCode::EDIABAS_IFH_0007,
    ErrorCode::EDIABAS_IFH_0008,
    ErrorCode::EDIABAS_IFH_0009,
    ErrorCode::EDIABAS_IFH_0010,
    ErrorCode::EDIABAS_IFH_0011,
    ErrorCode::EDIABAS_IFH_0012,
    ErrorCode::EDIABAS_IFH_0013,
    ErrorCode::EDIABAS_IFH_0014,
    ErrorCode::EDIABAS_IFH_0015,
    ErrorCode::EDIABAS_IFH_0016,
    ErrorCode::EDIABAS_BIP_0011,
    ErrorCode::EDIABAS_IFH_0069,
    ErrorCode::EDIABAS_IFH_0074,
];

/// C#: `(ErrorCodes)value` for `EDIABAS_RUN_0000..=EDIABAS_ERROR_LAST`
/// (values 250..=399, contiguous in the enum). Index = value - 250.
const RUN_TO_LAST: [ErrorCode; 150] = [
    ErrorCode::EDIABAS_RUN_0000,
    ErrorCode::EDIABAS_RUN_0001,
    ErrorCode::EDIABAS_RUN_0002,
    ErrorCode::EDIABAS_RUN_0003,
    ErrorCode::EDIABAS_RUN_0004,
    ErrorCode::EDIABAS_RUN_0005,
    ErrorCode::EDIABAS_RUN_0006,
    ErrorCode::EDIABAS_RUN_0007,
    ErrorCode::EDIABAS_RUN_0008,
    ErrorCode::EDIABAS_RUN_0009,
    ErrorCode::EDIABAS_RUN_0010,
    ErrorCode::EDIABAS_RUN_0011,
    ErrorCode::EDIABAS_RUN_0012,
    ErrorCode::EDIABAS_RUN_0013,
    ErrorCode::EDIABAS_RUN_0014,
    ErrorCode::EDIABAS_RUN_0015,
    ErrorCode::EDIABAS_RUN_0016,
    ErrorCode::EDIABAS_RUN_0017,
    ErrorCode::EDIABAS_RUN_0018,
    ErrorCode::EDIABAS_RUN_0019,
    ErrorCode::EDIABAS_RUN_0020,
    ErrorCode::EDIABAS_RUN_0021,
    ErrorCode::EDIABAS_RUN_0022,
    ErrorCode::EDIABAS_RUN_0023,
    ErrorCode::EDIABAS_RUN_0024,
    ErrorCode::EDIABAS_RUN_0025,
    ErrorCode::EDIABAS_RUN_0026,
    ErrorCode::EDIABAS_RUN_0027,
    ErrorCode::EDIABAS_RUN_0028,
    ErrorCode::EDIABAS_RUN_0029,
    ErrorCode::EDIABAS_RUN_0030,
    ErrorCode::EDIABAS_RUN_0031,
    ErrorCode::EDIABAS_RUN_0032,
    ErrorCode::EDIABAS_RUN_0033,
    ErrorCode::EDIABAS_RUN_0034,
    ErrorCode::EDIABAS_RUN_0035,
    ErrorCode::EDIABAS_RUN_0036,
    ErrorCode::EDIABAS_RUN_0037,
    ErrorCode::EDIABAS_RUN_0038,
    ErrorCode::EDIABAS_RUN_0039,
    ErrorCode::EDIABAS_RUN_0040,
    ErrorCode::EDIABAS_RUN_0041,
    ErrorCode::EDIABAS_RUN_0042,
    ErrorCode::EDIABAS_RUN_0043,
    ErrorCode::EDIABAS_RUN_0044,
    ErrorCode::EDIABAS_RUN_0045,
    ErrorCode::EDIABAS_RUN_0046,
    ErrorCode::EDIABAS_RUN_0047,
    ErrorCode::EDIABAS_RUN_0048,
    ErrorCode::EDIABAS_RUN_0049,
    ErrorCode::EDIABAS_RUN_0050,
    ErrorCode::EDIABAS_RUN_0051,
    ErrorCode::EDIABAS_RUN_0052,
    ErrorCode::EDIABAS_RUN_0053,
    ErrorCode::EDIABAS_RUN_0054,
    ErrorCode::EDIABAS_RUN_0055,
    ErrorCode::EDIABAS_RUN_0056,
    ErrorCode::EDIABAS_RUN_0057,
    ErrorCode::EDIABAS_RUN_0058,
    ErrorCode::EDIABAS_RUN_0059,
    ErrorCode::EDIABAS_RUN_0060,
    ErrorCode::EDIABAS_RUN_0061,
    ErrorCode::EDIABAS_RUN_0062,
    ErrorCode::EDIABAS_RUN_0063,
    ErrorCode::EDIABAS_RUN_0064,
    ErrorCode::EDIABAS_RUN_0065,
    ErrorCode::EDIABAS_RUN_0066,
    ErrorCode::EDIABAS_RUN_0067,
    ErrorCode::EDIABAS_RUN_0068,
    ErrorCode::EDIABAS_RUN_0069,
    ErrorCode::EDIABAS_RUN_0070,
    ErrorCode::EDIABAS_RUN_0071,
    ErrorCode::EDIABAS_RUN_0072,
    ErrorCode::EDIABAS_RUN_0073,
    ErrorCode::EDIABAS_RUN_0074,
    ErrorCode::EDIABAS_RUN_0075,
    ErrorCode::EDIABAS_RUN_0076,
    ErrorCode::EDIABAS_RUN_0077,
    ErrorCode::EDIABAS_RUN_0078,
    ErrorCode::EDIABAS_RUN_0079,
    ErrorCode::EDIABAS_RUN_0080,
    ErrorCode::EDIABAS_RUN_0081,
    ErrorCode::EDIABAS_RUN_0082,
    ErrorCode::EDIABAS_RUN_0083,
    ErrorCode::EDIABAS_RUN_0084,
    ErrorCode::EDIABAS_RUN_0085,
    ErrorCode::EDIABAS_RUN_0086,
    ErrorCode::EDIABAS_RUN_0087,
    ErrorCode::EDIABAS_RUN_0088,
    ErrorCode::EDIABAS_RUN_0089,
    ErrorCode::EDIABAS_RUN_0090,
    ErrorCode::EDIABAS_RUN_0091,
    ErrorCode::EDIABAS_RUN_0092,
    ErrorCode::EDIABAS_RUN_0093,
    ErrorCode::EDIABAS_RUN_0094,
    ErrorCode::EDIABAS_RUN_0095,
    ErrorCode::EDIABAS_RUN_0096,
    ErrorCode::EDIABAS_RUN_0097,
    ErrorCode::EDIABAS_RUN_0098,
    ErrorCode::EDIABAS_RUN_0099,
    ErrorCode::EDIABAS_SEC_0000,
    ErrorCode::EDIABAS_SEC_0001,
    ErrorCode::EDIABAS_SEC_0002,
    ErrorCode::EDIABAS_SEC_0003,
    ErrorCode::EDIABAS_SEC_0004,
    ErrorCode::EDIABAS_SEC_0005,
    ErrorCode::EDIABAS_SEC_0006,
    ErrorCode::EDIABAS_SEC_0007,
    ErrorCode::EDIABAS_SEC_0008,
    ErrorCode::EDIABAS_SEC_0009,
    ErrorCode::EDIABAS_SEC_0010,
    ErrorCode::EDIABAS_SEC_0011,
    ErrorCode::EDIABAS_SEC_0012,
    ErrorCode::EDIABAS_SEC_0013,
    ErrorCode::EDIABAS_SEC_0014,
    ErrorCode::EDIABAS_SEC_0015,
    ErrorCode::EDIABAS_SEC_0016,
    ErrorCode::EDIABAS_SEC_0017,
    ErrorCode::EDIABAS_SEC_0018,
    ErrorCode::EDIABAS_SEC_0019,
    ErrorCode::EDIABAS_SEC_0020,
    ErrorCode::EDIABAS_SEC_0021,
    ErrorCode::EDIABAS_SEC_0022,
    ErrorCode::EDIABAS_SEC_0023,
    ErrorCode::EDIABAS_SEC_0024,
    ErrorCode::EDIABAS_SEC_0025,
    ErrorCode::EDIABAS_SEC_0026,
    ErrorCode::EDIABAS_SEC_0027,
    ErrorCode::EDIABAS_SEC_0028,
    ErrorCode::EDIABAS_SEC_0029,
    ErrorCode::EDIABAS_SEC_0030,
    ErrorCode::EDIABAS_SEC_0031,
    ErrorCode::EDIABAS_SEC_0032,
    ErrorCode::EDIABAS_SEC_0033,
    ErrorCode::EDIABAS_SEC_0034,
    ErrorCode::EDIABAS_SEC_0035,
    ErrorCode::EDIABAS_SEC_0036,
    ErrorCode::EDIABAS_SEC_0037,
    ErrorCode::EDIABAS_SEC_0038,
    ErrorCode::EDIABAS_SEC_0039,
    ErrorCode::EDIABAS_SEC_0040,
    ErrorCode::EDIABAS_SEC_0041,
    ErrorCode::EDIABAS_SEC_0042,
    ErrorCode::EDIABAS_SEC_0043,
    ErrorCode::EDIABAS_SEC_0044,
    ErrorCode::EDIABAS_SEC_0045,
    ErrorCode::EDIABAS_SEC_0046,
    ErrorCode::EDIABAS_SEC_0047,
    ErrorCode::EDIABAS_SEC_0048,
    ErrorCode::EDIABAS_SEC_0049,
];

/// C#: `(ErrorCodes)value` followed by the range check of `OpGenerr`.
/// `None` for values outside `EDIABAS_RUN_0000..=EDIABAS_ERROR_LAST`.
fn error_code_from_value(value: u32) -> Option<ErrorCode> {
    let first = ErrorCode::EDIABAS_RUN_0000 as u32;
    let last = ErrorCode::EDIABAS_ERROR_LAST as u32;
    if value < first || value > last {
        return None;
    }
    Some(RUN_TO_LAST[(value - first) as usize])
}

/// C#: `OpErgb` (ergb)
pub(crate) fn op_ergb(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 1)? as u8 as i64;
    vm.set_result_data(ResultData::new(ResultType::B, name, ResultValue::Int(value)));
    Ok(())
}

/// C#: `OpErgw` (ergw)
pub(crate) fn op_ergw(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 2)? as u16 as i64;
    vm.set_result_data(ResultData::new(ResultType::W, name, ResultValue::Int(value)));
    Ok(())
}

/// C#: `OpErgd` (ergd)
pub(crate) fn op_ergd(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 4)? as u32 as i64;
    vm.set_result_data(ResultData::new(ResultType::D, name, ResultValue::Int(value)));
    Ok(())
}

/// C#: `OpErgi` (ergi). Sign extended from 16 bit, as C# `(Int64)(Int16)`.
pub(crate) fn op_ergi(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 2)? as u16 as i16 as i64;
    vm.set_result_data(ResultData::new(ResultType::I, name, ResultValue::Int(value)));
    Ok(())
}

/// C#: `OpErgr` (ergr)
pub(crate) fn op_ergr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_float(arg1)?;
    vm.set_result_data(ResultData::new(ResultType::R, name, ResultValue::Float(value)));
    Ok(())
}

/// C#: `OpErgs` (ergs)
pub(crate) fn op_ergs(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_string(arg1)?;
    vm.set_result_data(ResultData::new(ResultType::S, name, ResultValue::Str(value)));
    Ok(())
}

/// C#: `OpErgy` (ergy)
pub(crate) fn op_ergy(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_array(arg1)?;
    vm.set_result_data(ResultData::new(ResultType::Y, name, ResultValue::Bytes(value)));
    Ok(())
}

/// C#: `OpEnewset` (enewset). Moves the current result set into the list of
/// finished sets. C# does nothing when the set is empty.
pub(crate) fn op_enewset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !vm.results.is_empty() {
        let set = std::mem::take(&mut vm.results);
        vm.result_sets.push(set);
    }
    Ok(())
}

/// C#: `OpEtag` (etag). Jumps to arg0 if the result named by arg1 is not
/// requested. C# checks the request dict only when it is non-empty, and
/// reads arg0/arg1 only at that point; the same order is kept here.
pub(crate) fn op_etag(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if !vm.requested_results.is_empty() {
        let name = vm.op_string(arg1)?;
        if !vm.is_result_requested(&name) {
            vm.pc = vm.op_value(arg0, 0)?;
        }
    }
    Ok(())
}

/// C#: `OpEerr` (eerr). Raises the error that belongs to the trapped bit.
/// C# loops over `_trapBitDict` and `RaiseError` throws at the first match,
/// so the first matching key wins. No match raises EDIABAS_BIP_0000.
pub(crate) fn op_eerr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if vm.error_trap_bit_nr >= 0 {
        for key in TRAP_KEYS {
            if key.trap_bit() as i32 == vm.error_trap_bit_nr {
                return Err(vm.raise_error(key));
            }
        }
        return Err(vm.raise_error(ErrorCode::EDIABAS_BIP_0000));
    }
    Ok(())
}

/// C#: `OpErgc` (ergc). Sign extended from 8 bit, as C# `(Int64)(SByte)`.
pub(crate) fn op_ergc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 1)? as u8 as i8 as i64;
    vm.set_result_data(ResultData::new(ResultType::C, name, ResultValue::Int(value)));
    Ok(())
}

/// C#: `OpErgl` (ergl). Sign extended from 32 bit, as C# `(Int64)(Int32)`.
pub(crate) fn op_ergl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let name = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 4)? as u32 as i32 as i64;
    vm.set_result_data(ResultData::new(ResultType::L, name, ResultValue::Int(value)));
    Ok(())
}

/// C#: `OpErgsysi` (ergsysi). `!INITIALISIERUNG` sets `request_init` when the
/// value is non-zero and is not stored as a result. Other names go to the
/// system result set, zero extended (C# `(Int64)EdValueType`).
pub(crate) fn op_ergsysi(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let data = vm.op_string(arg0)?;
    let data_value = vm.op_value(arg1, 2)?;
    if data == "!INITIALISIERUNG" {
        if data_value != 0 {
            vm.request_init = true;
        }
    } else {
        vm.set_sys_result_data(ResultData::new(ResultType::I, data, ResultValue::Int(data_value as i64)));
    }
    Ok(())
}

/// C#: `OpGenerr` (generr). Raises the error code in arg0. Values outside
/// EDIABAS_RUN_0000..=EDIABAS_ERROR_LAST raise EDIABAS_BIP_0001.
pub(crate) fn op_generr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let value = vm.op_value(arg0, 0)?;
    match error_code_from_value(value) {
        Some(code) => Err(vm.raise_error(code)),
        None => Err(vm.raise_error(ErrorCode::EDIABAS_BIP_0001)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VmError;
    use crate::regs::Reg;

    fn oc() -> &'static OpCode {
        &crate::ops::OPCODES[0]
    }

    fn raised(r: VmResult<()>) -> Option<ErrorCode> {
        match r {
            Err(VmError::Raised(code)) => Some(code),
            _ => None,
        }
    }

    #[test]
    fn error_table_matches_enum_values() {
        for (i, code) in RUN_TO_LAST.iter().enumerate() {
            assert_eq!(*code as u32, 250 + i as u32);
        }
        assert_eq!(RUN_TO_LAST[149], ErrorCode::EDIABAS_ERROR_LAST);
    }

    #[test]
    fn ergb_stores_unsigned_byte() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"NAME"), Operand::imm8(0xFF));
        op_ergb(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("name").unwrap();
        assert_eq!(r.ty, ResultType::B);
        assert_eq!(r.value, ResultValue::Int(255));
    }

    #[test]
    fn ergw_stores_unsigned_word() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"W"), Operand::imm16(0xFFFF));
        op_ergw(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("W").unwrap();
        assert_eq!(r.ty, ResultType::W);
        assert_eq!(r.value, ResultValue::Int(65535));
    }

    #[test]
    fn ergd_stores_unsigned_dword() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"D"), Operand::imm32(0x8000_0000));
        op_ergd(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("D").unwrap();
        assert_eq!(r.ty, ResultType::D);
        assert_eq!(r.value, ResultValue::Int(2_147_483_648));
    }

    #[test]
    fn ergi_sign_extends_word() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"I"), Operand::imm16(0x8000));
        op_ergi(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("I").unwrap();
        assert_eq!(r.ty, ResultType::I);
        assert_eq!(r.value, ResultValue::Int(-32768));
    }

    #[test]
    fn ergr_stores_float() {
        let mut vm = Vm::for_test();
        vm.regs.set_float(Reg::F(0), 1.5).unwrap();
        let (mut a0, mut a1) = (Operand::imm_str(b"R"), Operand::r(Reg::F(0)));
        op_ergr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("R").unwrap();
        assert_eq!(r.ty, ResultType::R);
        assert_eq!(r.value, ResultValue::Float(1.5));
    }

    #[test]
    fn ergs_stores_string_up_to_nul() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"S"), Operand::imm_str(b"hi\0junk"));
        op_ergs(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("S").unwrap();
        assert_eq!(r.ty, ResultType::S);
        assert_eq!(r.value, ResultValue::Str("hi".into()));
    }

    #[test]
    fn ergy_stores_bytes() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"Y"), Operand::imm_str(&[1, 2, 3]));
        op_ergy(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("Y").unwrap();
        assert_eq!(r.ty, ResultType::Y);
        assert_eq!(r.value, ResultValue::Bytes(vec![1, 2, 3]));
    }

    #[test]
    fn ergc_sign_extends_byte() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"C"), Operand::imm8(0xFF));
        op_ergc(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("C").unwrap();
        assert_eq!(r.ty, ResultType::C);
        assert_eq!(r.value, ResultValue::Int(-1));
    }

    #[test]
    fn ergl_sign_extends_dword() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"L"), Operand::imm32(0xFFFF_FFFF));
        op_ergl(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let r = vm.results.get("L").unwrap();
        assert_eq!(r.ty, ResultType::L);
        assert_eq!(r.value, ResultValue::Int(-1));
    }

    #[test]
    fn ergsysi_stores_system_result() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"SYS"), Operand::imm16(5));
        op_ergsysi(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.results.is_empty());
        let r = vm.sys_results.get("SYS").unwrap();
        assert_eq!(r.ty, ResultType::I);
        assert_eq!(r.value, ResultValue::Int(5));
    }

    #[test]
    fn ergsysi_initialisierung_sets_request_init() {
        let mut vm = Vm::for_test();
        vm.request_init = false;
        let (mut a0, mut a1) = (Operand::imm_str(b"!INITIALISIERUNG"), Operand::imm16(0));
        op_ergsysi(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(!vm.request_init);
        assert!(vm.sys_results.is_empty());

        let mut a1 = Operand::imm16(1);
        op_ergsysi(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.request_init);
        assert!(vm.sys_results.is_empty());
    }

    #[test]
    fn enewset_moves_current_set() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b"A"), Operand::imm8(1));
        op_ergb(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        op_enewset(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.results.is_empty());
        assert_eq!(vm.result_sets.len(), 1);
        assert_eq!(vm.result_sets[0].get("A").unwrap().value, ResultValue::Int(1));
    }

    #[test]
    fn enewset_empty_set_is_noop() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::none(), Operand::none());
        op_enewset(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.result_sets.is_empty());
    }

    #[test]
    fn etag_jumps_when_result_not_requested() {
        let mut vm = Vm::for_test();
        vm.pc = 0x10;
        vm.requested_results = vec!["AAA".into()];
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::imm_str(b"bbb"));
        op_etag(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x200);
    }

    #[test]
    fn etag_no_jump_when_requested_or_all_requested() {
        let mut vm = Vm::for_test();
        vm.pc = 0x10;
        vm.requested_results = vec!["AAA".into()];
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::imm_str(b"aaa"));
        op_etag(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x10);

        vm.requested_results.clear();
        op_etag(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x10);
    }

    #[test]
    fn eerr_without_trap_is_noop() {
        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = -1;
        let (mut a0, mut a1) = (Operand::none(), Operand::none());
        assert!(op_eerr(&mut vm, oc(), &mut a0, &mut a1).is_ok());
    }

    #[test]
    fn eerr_raises_code_for_trap_bit() {
        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 6;
        let (mut a0, mut a1) = (Operand::none(), Operand::none());
        assert_eq!(raised(op_eerr(&mut vm, oc(), &mut a0, &mut a1)), Some(ErrorCode::EDIABAS_BIP_0006));
    }

    #[test]
    fn eerr_unmapped_bit_raises_bip_0000() {
        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 0;
        let (mut a0, mut a1) = (Operand::none(), Operand::none());
        assert_eq!(raised(op_eerr(&mut vm, oc(), &mut a0, &mut a1)), Some(ErrorCode::EDIABAS_BIP_0000));
    }

    #[test]
    fn generr_raises_code_in_range() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm32(ErrorCode::EDIABAS_RUN_0005 as u32), Operand::none());
        assert_eq!(raised(op_generr(&mut vm, oc(), &mut a0, &mut a1)), Some(ErrorCode::EDIABAS_RUN_0005));
        assert_eq!(vm.error_code_last, ErrorCode::EDIABAS_RUN_0005);
    }

    #[test]
    fn generr_upper_bound_is_last_code() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm32(ErrorCode::EDIABAS_ERROR_LAST as u32), Operand::none());
        assert_eq!(raised(op_generr(&mut vm, oc(), &mut a0, &mut a1)), Some(ErrorCode::EDIABAS_ERROR_LAST));
    }

    #[test]
    fn generr_out_of_range_raises_bip_0001() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm32(0), Operand::none());
        assert_eq!(raised(op_generr(&mut vm, oc(), &mut a0, &mut a1)), Some(ErrorCode::EDIABAS_BIP_0001));
        let (mut a0, mut a1) = (Operand::imm32(400), Operand::none());
        assert_eq!(raised(op_generr(&mut vm, oc(), &mut a0, &mut a1)), Some(ErrorCode::EDIABAS_BIP_0001));
    }
}
