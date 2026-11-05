//! Opcodes: parb, parw, parl, pars, parr, pary, parn.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpParl` (parb, parw, parl). The C# handler is shared by all three
/// opcodes and does not look at the opcode name, so neither does this port.
pub(crate) fn op_parl(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpParl Invalid type");
    }

    let mut result: u32 = 0;
    vm.flags.zero = true;
    vm.flags.carry = false;
    vm.flags.sign = false;
    vm.flags.overflow = false;
    // C# `pos--` on uint: 0 wraps to 0xFFFFFFFF, which is out of range below.
    let pos = vm.op_value(arg1, 0)?.wrapping_sub(1);

    let arg_strings = vm.active_arg_strings();
    if (pos as usize) < arg_strings.len() {
        let arg_string = &arg_strings[pos as usize];
        if !arg_string.is_empty() {
            // C# casts Int64 to uint: truncation.
            result = string_to_value(arg_string).0 as u32;
            vm.flags.zero = false;
        }
    }
    vm.op_set_raw(arg0, Value::Int(result), 1)
}

/// C#: `OpPars` (pars)
pub(crate) fn op_pars(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpPars: Invalid type");
    }

    let mut result = String::new();
    vm.flags.zero = true;
    let pos = vm.op_value(arg1, 0)?.wrapping_sub(1);

    let arg_strings = vm.active_arg_strings();
    if (pos as usize) < arg_strings.len() {
        let arg_string = &arg_strings[pos as usize];
        if !arg_string.is_empty() {
            result = arg_string.clone();
            vm.flags.zero = false;
        }
    }
    vm.op_set_string(arg0, &result)
}

/// C#: `OpParr` (parr)
pub(crate) fn op_parr(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpParr: Invalid type");
    }

    let mut result: f64 = 0.0;
    vm.flags.zero = true;
    vm.flags.carry = false;
    vm.flags.sign = false;
    vm.flags.overflow = false;
    let pos = vm.op_value(arg1, 0)?.wrapping_sub(1);

    let arg_strings = vm.active_arg_strings();
    if (pos as usize) < arg_strings.len() {
        let arg_string = &arg_strings[pos as usize];
        if !arg_string.is_empty() {
            result = string_to_float(arg_string).0;
            vm.flags.zero = false;
        }
    }
    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpPary` (pary)
pub(crate) fn op_pary(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpPary: Invalid type");
    }

    let mut result: Vec<u8> = Vec::new();
    vm.flags.zero = true;
    let arg_bin = vm.active_arg_binary().to_vec();
    if !arg_bin.is_empty() {
        result = arg_bin;
        vm.flags.zero = false;
    }
    vm.op_set_array(arg0, result)
}

/// C#: `OpParn` (parn)
pub(crate) fn op_parn(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpParn: Invalid type");
    }

    let count = vm.active_arg_strings().len() as u32;
    vm.op_set_raw(arg0, Value::Int(count), 1)?;
    vm.flags.overflow = false;
    let value = vm.op_value(arg0, 0)?;
    let len = vm.op_data_len(arg0, false)?;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `EdiabasNet.StringToValue(string, out bool valid)`. Returns `(value, valid)`.
/// Accepts "0x" hex, "0y" binary and decimal (a trailing part after '.' or ','
/// is dropped). Any parse failure gives 0 and `valid == false`.
fn string_to_value(number: &str) -> (i64, bool) {
    let number_local = number.trim_end();
    if number_local.is_empty() {
        return (0, false);
    }
    let number_lower = number_local.to_lowercase();
    if let Some(hex) = number_lower.strip_prefix("0x") {
        // hex: the char after "0x" must be a digit or a-f
        let first = hex.chars().next();
        if first.is_some_and(|c| c.is_ascii_hexdigit())
            && let Some(v) = parse_radix_i64(&number_local[2..], 16)
        {
            return (v, true);
        }
        (0, false)
    } else if number_lower.starts_with("0y") {
        // binary
        match parse_radix_i64(&number_local[2..], 2) {
            Some(v) => (v, true),
            None => (0, false),
        }
    } else {
        // decimal
        if number_lower == "-" || number_lower == "--" {
            return (0, false);
        }
        if number_local
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic())
        {
            return (0, false);
        }
        let conv = number_local.trim_start();
        let conv = match conv.find(['.', ',']) {
            Some(index) => &conv[..index],
            None => conv,
        };
        match conv.parse::<i64>() {
            Ok(v) => (v, true),
            Err(_) => (0, false),
        }
    }
}

/// Integer parse like .NET `Convert.ToInt64(s, radix)` for radix 2 and 16.
/// Hex accepts an optional "0x" prefix. Values are two's complement, so
/// "FFFFFFFFFFFFFFFF" gives -1. Leading zeros are accepted here; .NET may
/// reject more than 16 hex digits even when they are zeros (not verified).
fn parse_radix_i64(s: &str, radix: u32) -> Option<i64> {
    let s = if radix == 16 {
        s.strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s)
    } else {
        s
    };
    if s.is_empty() || !s.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    u64::from_str_radix(s, radix).ok().map(|v| v as i64)
}

/// C#: `EdiabasNet.StringToFloat(string, out bool valid)`. Returns `(value, valid)`.
fn string_to_float(number: &str) -> (f64, bool) {
    // C# rejects strings that do not survive an ASCII round trip.
    if !number.is_ascii() {
        return (0.0, false);
    }
    // Decimal comma is accepted as decimal point.
    let s = number.replace(',', ".");
    let s = s.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'));
    match s.parse::<f64>() {
        Ok(v) => (v, true),
        Err(_) => (0.0, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regs::Reg;

    fn oc() -> &'static OpCode {
        &crate::ops::OPCODES[0]
    }

    fn vm_with_args(args: &[u8]) -> Vm {
        let mut vm = Vm::for_test();
        vm.args.binary = args.to_vec();
        vm
    }

    #[test]
    fn parl_reads_arg_by_position() {
        let mut vm = vm_with_args(b"12;0x1F;abc");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm8(2));
        op_parl(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0x1F);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn parl_non_numeric_arg_gives_zero_with_zero_flag_clear() {
        // C# quirk: a non-empty but unparsable arg gives 0 and clears Zero.
        let mut vm = vm_with_args(b"12;0x1F;abc");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm8(3));
        op_parl(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn parl_missing_arg_sets_zero() {
        let mut vm = vm_with_args(b"12;0x1F");
        vm.flags.carry = true;
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm8(5));
        op_parl(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);
        assert!(vm.flags.zero);
        assert!(!vm.flags.carry);
    }

    #[test]
    fn parl_position_zero_wraps_to_out_of_range() {
        let mut vm = vm_with_args(b"12");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm8(0));
        op_parl(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.zero);
    }

    #[test]
    fn parl_rejects_non_register_arg0() {
        let mut vm = vm_with_args(b"12");
        let (mut a0, mut a1) = (Operand::imm8(1), Operand::imm8(1));
        assert!(op_parl(&mut vm, oc(), &mut a0, &mut a1).is_err());
    }

    #[test]
    fn parb_writes_byte_register() {
        let mut vm = vm_with_args(b"12;0x1F");
        let (mut a0, mut a1) = (Operand::r(Reg::B(0)), Operand::imm8(1));
        op_parl(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::B(0)).unwrap(), 12);
    }

    #[test]
    fn parn_counts_args() {
        let mut vm = vm_with_args(b"a;b;c");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm8(0));
        op_parn(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 3);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn parn_no_args_gives_zero_and_sets_zero_flag() {
        let mut vm = vm_with_args(b"");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm8(0));
        op_parn(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);
        assert!(vm.flags.zero);
    }

    #[test]
    fn pars_reads_string_arg() {
        let mut vm = vm_with_args(b"12;;abc");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm8(3));
        op_pars(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_string(&Operand::r(Reg::S(0))).unwrap(), "abc");
        assert!(!vm.flags.zero);
    }

    #[test]
    fn pars_empty_arg_sets_zero() {
        let mut vm = vm_with_args(b"12;;abc");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm8(2));
        op_pars(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_string(&Operand::r(Reg::S(0))).unwrap(), "");
        assert!(vm.flags.zero);
    }

    #[test]
    fn parr_reads_float_arg() {
        let mut vm = vm_with_args(b"1.5;,25;x");
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm8(1));
        op_parr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 1.5);
        assert!(!vm.flags.zero);

        let mut a1 = Operand::imm8(2);
        op_parr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 0.25);
    }

    #[test]
    fn parr_missing_arg_sets_zero() {
        let mut vm = vm_with_args(b"1.5");
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm8(9));
        op_parr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 0.0);
        assert!(vm.flags.zero);
    }

    #[test]
    fn pary_reads_binary_arg() {
        let mut vm = vm_with_args(&[1, 2, 3]);
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm8(0));
        op_pary(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&Operand::r(Reg::S(0))).unwrap(), vec![1, 2, 3]);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn pary_empty_gives_zero_flag() {
        let mut vm = vm_with_args(b"");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm8(0));
        op_pary(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.op_array(&Operand::r(Reg::S(0))).unwrap().is_empty());
        assert!(vm.flags.zero);
    }

    #[test]
    fn string_to_value_formats() {
        assert_eq!(string_to_value("0x1F"), (31, true));
        assert_eq!(string_to_value("0X1f"), (31, true));
        assert_eq!(string_to_value("0y101"), (5, true));
        assert_eq!(string_to_value("-5"), (-5, true));
        assert_eq!(string_to_value("12.9"), (12, true));
        assert_eq!(string_to_value("12,9"), (12, true));
        assert_eq!(string_to_value("  7"), (7, true));
        assert_eq!(string_to_value("0xFFFFFFFFFFFFFFFF"), (-1, true));
        assert_eq!(string_to_value("abc"), (0, false));
        assert_eq!(string_to_value("0x"), (0, false));
        assert_eq!(string_to_value("--"), (0, false));
        assert_eq!(string_to_value("   "), (0, false));
    }

    #[test]
    fn string_to_float_formats() {
        assert_eq!(string_to_float("1,5"), (1.5, true));
        assert_eq!(string_to_float("1.5 "), (1.5, true));
        assert_eq!(string_to_float("x"), (0.0, false));
        assert_eq!(string_to_float("1e3"), (1000.0, true));
    }
}
