//! Opcodes: a2flt, fadd, fsub, fmul, fdiv, fix2flt, flt2a, setflt, flt2fix, flt2y4, flt2y8, y42flt, y82flt, fcomp.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug)]
pub struct State {
    /// C#: `_floatPrecision` (EdiabasNet.cs: `= 4`).
    pub float_precision: u32,
}

impl Default for State {
    fn default() -> Self {
        Self { float_precision: 4 }
    }
}

/// C#: `EdiabasNet.EdiabasVersion` (0x773). `IsMinVersion760` is always true.
const EDIABAS_VERSION: u32 = 0x773;
const IS_MIN_VERSION_760: bool = EDIABAS_VERSION >= 0x760;

/// C#: `OpA2Flt` (a2flt)
pub(crate) fn op_a2flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpA2flt: Invalid type");
    }

    let value_str = vm.op_string(arg1)?;
    let (result, valid) = string_to_float(&value_str);
    if IS_MIN_VERSION_760 && !valid {
        let mut compat_mode: i32 = 0;
        if let Some(prop) = vm.config_property("CompatMode") {
            compat_mode = string_to_value(prop).0 as i32;
        }
        if compat_mode == 0 {
            return Err(vm.raise_error(ErrorCode::EDIABAS_BIP_0011));
        }
    }

    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpFadd` (fadd)
pub(crate) fn op_fadd(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFadd Invalid type");
    }

    let val0 = vm.op_float(arg0)?;
    let val1 = vm.op_float(arg1)?;

    // C# wraps SetError in try/catch(Exception); an untrapped SetError
    // throws there, which the catch repeats, so the error propagates.
    let result = val0 + val1;
    if !result.is_finite() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0011)?;
    }

    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpFsub` (fsub)
pub(crate) fn op_fsub(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFsub: Invalid type");
    }

    let val0 = vm.op_float(arg0)?;
    let val1 = vm.op_float(arg1)?;

    let result = val0 - val1;
    if !result.is_finite() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0011)?;
    }

    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpFmul` (fmul)
pub(crate) fn op_fmul(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFmul: Invalid type");
    }

    let val0 = vm.op_float(arg0)?;
    let val1 = vm.op_float(arg1)?;

    let result = val0 * val1;
    if !result.is_finite() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0011)?;
    }

    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpFdiv` (fdiv)
pub(crate) fn op_fdiv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFdiv: Invalid type");
    }

    let val0 = vm.op_float(arg0)?;
    let val1 = vm.op_float(arg1)?;

    // Division by zero gives Infinity or NaN in IEEE, as in C#.
    let result = val0 / val1;
    if !result.is_finite() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0011)?;
    }

    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpFix2Flt` (fix2flt)
pub(crate) fn op_fix2flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFix2flt: Invalid type");
    }

    let value = vm.op_value(arg1, 0)?;
    let len = vm.op_data_len(arg1, false)?;
    let result: f64 = match len {
        1 => (value as u8 as i8) as f64,
        2 => (value as u16 as i16) as f64,
        4 => (value as i32) as f64,
        _ => return internal("OpFix2flt: Invalid length"),
    };

    vm.op_set_raw(arg0, Value::Float(result), 1)
}

/// C#: `OpFlt2A` (flt2a)
pub(crate) fn op_flt2a(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFlt2a: Invalid type");
    }

    let value = vm.op_float(arg1)?;
    let precision = vm.st_float.float_precision;
    let value_conv = round_to_significant_digits(value, precision)?;

    let mut result = format_double(value_conv);
    // Truncate after the precision-th digit (no rounding here).
    let mut digit_count: u32 = 0;
    let mut cut = None;
    for (pos, c) in result.char_indices() {
        if c.is_ascii_digit() {
            digit_count += 1;
            if digit_count >= precision {
                cut = Some(pos + 1);
                break;
            }
        }
    }
    if let Some(end) = cut {
        result.truncate(end);
    }

    vm.op_set_string(arg0, &result)
}

/// C#: `OpSetflt` (setflt)
pub(crate) fn op_setflt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    vm.st_float.float_precision = vm.op_value(arg0, 0)?;
    Ok(())
}

/// C#: `OpFlt2Fix` (flt2fix)
pub(crate) fn op_flt2fix(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFlt2fix: Invalid type");
    }

    let value = vm.op_float(arg1)?;
    // C# `(uint)double`: saturating on .NET 9+ (NaN and negatives give 0).
    let result = value as u32;
    vm.op_set_raw(arg0, Value::Int(result), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(result, 4);
    Ok(())
}

/// C#: `OpFlt2Y4` (flt2y4), real_to_data, little endian.
pub(crate) fn op_flt2y4(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let Some(reg) = arg0.reg() else {
        return internal("OpFlt2y4: Invalid type");
    };

    let mut data_array_dest = vm.regs.string(reg)?.get(false);
    let start_idx = match arg0.d2 {
        OpData::Val(v) => v,
        _ => return internal("OpFlt2y4: invalid index data type"),
    };

    let data_len: u32 = 4;
    if start_idx.wrapping_add(data_len) > vm.array_max_size() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0001)?;
        return Ok(());
    }
    let end = (start_idx + data_len) as usize;
    if data_array_dest.len() < end {
        data_array_dest.resize(end, 0);
    }

    let value = vm.op_float(arg1)?;
    // C# `(Single)double` never throws, so its try/catch is dropped.
    let single_value = value as f32;
    data_array_dest[start_idx as usize..end].copy_from_slice(&single_value.to_le_bytes());
    vm.reg_set_array(reg, &data_array_dest, false)
}

/// C#: `OpFlt2Y8` (flt2y8), real_to_data, little endian.
pub(crate) fn op_flt2y8(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let Some(reg) = arg0.reg() else {
        return internal("OpFlt2y8: Invalid type");
    };

    let mut data_array_dest = vm.regs.string(reg)?.get(false);
    let start_idx = match arg0.d2 {
        OpData::Val(v) => v,
        _ => return internal("OpFlt2y8: invalid index data type"),
    };

    let data_len: u32 = 8;
    if start_idx.wrapping_add(data_len) > vm.array_max_size() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0001)?;
        return Ok(());
    }
    let end = (start_idx + data_len) as usize;
    if data_array_dest.len() < end {
        data_array_dest.resize(end, 0);
    }

    let value = vm.op_float(arg1)?;
    data_array_dest[start_idx as usize..end].copy_from_slice(&value.to_le_bytes());
    vm.reg_set_array(reg, &data_array_dest, false)
}

/// C#: `OpY42Flt` (y42flt), data_to_real, little endian.
pub(crate) fn op_y42flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("Opy42flt: Invalid type");
    }

    let data_array = vm.op_array(arg1)?;
    // C# BitConverter.ToSingle throws when fewer than 4 bytes are present.
    if data_array.len() < 4 {
        return internal("BitConverter.ToSingle: index out of range");
    }
    let value = f32::from_le_bytes([data_array[0], data_array[1], data_array[2], data_array[3]]) as f64;
    vm.op_set_raw(arg0, Value::Float(value), 1)
}

/// C#: `OpY82Flt` (y82flt), data_to_real, little endian.
pub(crate) fn op_y82flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpY82flt: Invalid type");
    }

    let data_array = vm.op_array(arg1)?;
    // C# BitConverter.ToDouble throws when fewer than 8 bytes are present.
    if data_array.len() < 8 {
        return internal("BitConverter.ToDouble: index out of range");
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data_array[..8]);
    vm.op_set_raw(arg0, Value::Float(f64::from_le_bytes(bytes)), 1)
}

/// C#: `OpFcomp` (fcomp)
pub(crate) fn op_fcomp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let val0 = vm.op_float(arg0)?;
    let val1 = vm.op_float(arg1)?;

    let diff = val0 - val1;
    if !diff.is_finite() {
        vm.flags.carry = true;
    }
    vm.flags.zero = val0 == val1;
    vm.flags.sign = val0 < val1;
    vm.flags.overflow = false;
    Ok(())
}

/// C#: `RoundToSignificantDigits`.
fn round_to_significant_digits(value: f64, digits: u32) -> VmResult<f64> {
    if value == 0.0 {
        return Ok(0.0);
    }
    let scale = 10f64.powf(value.abs().log10().floor() + 1.0);
    Ok(scale * dotnet_round(value / scale, digits)?)
}

/// C#: `Math.Round(double, int)` (MidpointRounding.ToEven).
fn dotnet_round(value: f64, digits: u32) -> VmResult<f64> {
    const POW10: [f64; 16] = [
        1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
    ];
    // C# throws ArgumentOutOfRangeException for digits > 15.
    if digits > 15 {
        return internal("Math.Round: digits out of range");
    }
    if value.abs() < 1e16 {
        let power10 = POW10[digits as usize];
        Ok((value * power10).round_ties_even() / power10)
    } else {
        Ok(value)
    }
}

/// C#: `string.Format(InvariantCulture, "{0}", double)` (.NET Core 3.0+:
/// shortest round-trip, "G" style). Scientific notation is used when the
/// decimal exponent is above 15 (or the digit count) or below -4.
fn format_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.to_string();
    }

    // Rust `{:e}` gives the shortest round-trip digits: "3.142e0".
    let sci = format!("{:e}", v.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    // Value = 0.DIGITS * 10^scale (the .NET "Scale").
    let scale = exp + 1;
    let max_digits = digits.len().max(15) as i32;

    let mut out = String::new();
    if v < 0.0 {
        out.push('-');
    }
    if scale > max_digits || scale < -3 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let e = scale - 1;
        out.push_str(&format!("E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs()));
    } else if scale <= 0 {
        out.push_str("0.");
        for _ in 0..(-scale) {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        let s = scale as usize;
        if digits.len() <= s {
            out.push_str(&digits);
            for _ in 0..(s - digits.len()) {
                out.push('0');
            }
        } else {
            out.push_str(&digits[..s]);
            out.push('.');
            out.push_str(&digits[s..]);
        }
    }
    out
}

/// C#: `EdiabasNet.StringToFloat(number, out valid)`.
/// Non-ASCII input is invalid. Commas become dots. The rest follows
/// `double.Parse` with InvariantCulture (NumberStyles.Float).
fn string_to_float(number: &str) -> (f64, bool) {
    if !number.is_ascii() {
        return (0.0, false);
    }
    let replaced = number.replace(',', ".");
    match parse_double_invariant(&replaced) {
        Some(v) => (v, true),
        None => (0.0, false),
    }
}

/// `double.Parse(s, NumberStyles.Float | AllowThousands, InvariantCulture)`.
/// Returns None where .NET throws FormatException.
fn parse_double_invariant(s: &str) -> Option<f64> {
    let t = s.trim_matches(|c: char| matches!(c, '\t'..='\r' | ' '));
    let (neg, body) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if body.eq_ignore_ascii_case("Infinity") {
        return Some(if neg { f64::NEG_INFINITY } else { f64::INFINITY });
    }
    if body.eq_ignore_ascii_case("NaN") {
        return Some(f64::NAN);
    }

    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_part = &body[..i];
    let mut frac = "";
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac = &body[fs..i];
    }
    if int_part.is_empty() && frac.is_empty() {
        return None;
    }
    let mut exp = String::from("0");
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        let mut exp_neg = false;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            exp_neg = b[i] == b'-';
            i += 1;
        }
        let es = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            return None;
        }
        exp = format!("{}{}", if exp_neg { "-" } else { "" }, &body[es..i]);
    }
    if i != b.len() {
        return None;
    }

    let normalized = format!(
        "{}{}.{}e{}",
        if neg { "-" } else { "" },
        if int_part.is_empty() { "0" } else { int_part },
        if frac.is_empty() { "0" } else { frac },
        exp
    );
    normalized.parse::<f64>().ok()
}

/// C#: `EdiabasNet.StringToValue(number, out valid)`, reduced to what the
/// float handlers need (`CompatMode`). Returns (value, valid).
fn string_to_value(number: &str) -> (i64, bool) {
    let number_local = number.trim_end();
    if number_local.is_empty() {
        return (0, false);
    }
    let lower = number_local.to_ascii_lowercase();
    if lower.starts_with("0x") {
        // hex: third char must be a hex digit
        if lower.len() > 2 && lower.as_bytes()[2].is_ascii_hexdigit() {
            return match parse_int_radix(&number_local[2..], 16) {
                Some(v) => (v, true),
                None => (0, false),
            };
        }
        (0, false)
    } else if lower.starts_with("0y") {
        match parse_int_radix(&number_local[2..], 2) {
            Some(v) => (v, true),
            None => (0, false),
        }
    } else if lower != "-" && lower != "--" && !lower.as_bytes()[0].is_ascii_alphabetic() {
        let trimmed = number_local.trim_start();
        let conv = match trimmed.find(['.', ',']) {
            Some(idx) => &trimmed[..idx],
            None => trimmed,
        };
        match parse_int_radix(conv, 10) {
            Some(v) => (v, true),
            None => (0, false),
        }
    } else {
        (0, false)
    }
}

/// C#: `Convert.ToInt64(s, base)` for bases 2, 10 and 16. Returns None on
/// FormatException or OverflowException.
fn parse_int_radix(s: &str, radix: u32) -> Option<i64> {
    if radix == 10 {
        let (neg, digits) = match s.as_bytes().first() {
            Some(b'-') => (true, &s[1..]),
            Some(b'+') => (false, &s[1..]),
            _ => (false, s),
        };
        let digits = digits.trim_end();
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let v: i128 = digits.parse().ok()?;
        let v = if neg { -v } else { v };
        return i64::try_from(v).ok();
    }
    // Hex and binary: unsigned digits, wrapped into i64 like C#.
    let digits = s.trim_end();
    let max_digits = if radix == 16 { 16 } else { 64 };
    if digits.is_empty() || digits.len() > max_digits {
        return None;
    }
    let mut acc: u64 = 0;
    for c in digits.chars() {
        let d = c.to_digit(radix)? as u64;
        acc = acc.checked_mul(radix as u64)?.checked_add(d)?;
    }
    Some(acc as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regs::Reg;

    fn oc() -> &'static OpCode {
        &crate::ops::OPCODES[0]
    }

    fn vm() -> Vm {
        let mut vm = Vm::for_test();
        vm.set_array_max_buf_size(64);
        vm
    }

    fn is_raised(r: VmResult<()>) -> bool {
        matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0011)))
    }

    #[test]
    fn a2flt_parses_and_accepts_comma() {
        let mut vm = vm();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm_str(b"3.5"));
        op_a2flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 3.5);

        let mut a1 = Operand::imm_str(b"1,5");
        op_a2flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 1.5);
    }

    #[test]
    fn a2flt_invalid_raises_unless_compat_mode() {
        let mut vm = vm();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm_str(b"abc"));
        assert!(is_raised(op_a2flt(&mut vm, oc(), &mut a0, &mut a1)));

        vm.config.insert("CompatMode".into(), "1".into());
        op_a2flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 0.0);
    }

    #[test]
    fn fadd_adds_and_overflow_raises() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.5).unwrap();
        vm.regs.set_float(Reg::F(1), 2.25).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::r(Reg::F(1)));
        op_fadd(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 3.75);

        vm.regs.set_float(Reg::F(0), f64::MAX).unwrap();
        vm.regs.set_float(Reg::F(1), f64::MAX).unwrap();
        assert!(is_raised(op_fadd(&mut vm, oc(), &mut a0, &mut a1)));
    }

    #[test]
    fn fsub_subtracts() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.0).unwrap();
        vm.regs.set_float(Reg::F(1), 0.25).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::r(Reg::F(1)));
        op_fsub(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 0.75);
    }

    #[test]
    fn fmul_multiplies() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 3.0).unwrap();
        vm.regs.set_float(Reg::F(1), -2.0).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::r(Reg::F(1)));
        op_fmul(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), -6.0);
    }

    #[test]
    fn fdiv_divides_and_zero_divisor_raises() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.0).unwrap();
        vm.regs.set_float(Reg::F(1), 4.0).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::r(Reg::F(1)));
        op_fdiv(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 0.25);

        vm.regs.set_float(Reg::F(1), 0.0).unwrap();
        assert!(is_raised(op_fdiv(&mut vm, oc(), &mut a0, &mut a1)));
    }

    #[test]
    fn fix2flt_sign_extends_by_length() {
        let mut vm = vm();
        let mut a0 = Operand::r(Reg::F(0));
        let mut a1 = Operand::imm8(0xFF);
        op_fix2flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), -1.0);

        let mut a1 = Operand::imm16(0x8000);
        op_fix2flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), -32768.0);

        let mut a1 = Operand::imm32(0xFFFF_FFFE);
        op_fix2flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), -2.0);
    }

    #[test]
    fn flt2a_formats_with_precision() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 3.14159).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::r(Reg::F(0)));
        op_flt2a(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_string(&Operand::r(Reg::S(0))).unwrap(), "3.142");

        vm.st_float.float_precision = 2;
        op_flt2a(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        // Rounds to 2 significant digits (3.1), not 3.14 as the task note suggested.
        assert_eq!(vm.op_string(&Operand::r(Reg::S(0))).unwrap(), "3.1");
    }

    #[test]
    fn flt2a_zero_and_negative() {
        let mut vm = vm();
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::r(Reg::F(0)));
        vm.regs.set_float(Reg::F(0), 0.0).unwrap();
        op_flt2a(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_string(&Operand::r(Reg::S(0))).unwrap(), "0");

        vm.regs.set_float(Reg::F(0), -2.5).unwrap();
        op_flt2a(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_string(&Operand::r(Reg::S(0))).unwrap(), "-2.5");
    }

    #[test]
    fn format_double_matches_dotnet_g() {
        assert_eq!(format_double(1e15), "1E+15");
        assert_eq!(format_double(1e14), "100000000000000");
        assert_eq!(format_double(0.0001), "0.0001");
        assert_eq!(format_double(0.00001), "1E-05");
        assert_eq!(format_double(1.5e-7), "1.5E-07");
        assert_eq!(format_double(f64::INFINITY), "Infinity");
        assert_eq!(format_double(f64::NAN), "NaN");
    }

    #[test]
    fn flt2fix_truncates_and_sets_flags() {
        let mut vm = vm();
        vm.flags.overflow = true;
        vm.regs.set_float(Reg::F(0), 12.9).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::r(Reg::F(0)));
        op_flt2fix(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 12);
        assert!(!vm.flags.overflow);
        assert!(!vm.flags.zero);

        vm.regs.set_float(Reg::F(0), -1.0).unwrap();
        op_flt2fix(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        // C# saturates negative doubles to 0 when cast to uint.
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);
        assert!(vm.flags.zero);
    }

    #[test]
    fn setflt_sets_precision() {
        let mut vm = vm();
        assert_eq!(vm.st_float.float_precision, 4);
        let (mut a0, mut a1) = (Operand::imm8(7), Operand::none());
        op_setflt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.st_float.float_precision, 7);
    }

    #[test]
    fn flt2y4_writes_little_endian_float32() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.0).unwrap();
        let (mut a0, mut a1) = (Operand::idx(Reg::S(0), 2), Operand::r(Reg::F(0)));
        op_flt2y4(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false), vec![0, 0, 0, 0, 0x80, 0x3F]);
    }

    #[test]
    fn flt2y4_too_large_index_raises() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.0).unwrap();
        let (mut a0, mut a1) = (Operand::idx(Reg::S(0), 61), Operand::r(Reg::F(0)));
        assert!(matches!(
            op_flt2y4(&mut vm, oc(), &mut a0, &mut a1),
            Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0001))
        ));
    }

    #[test]
    fn flt2y8_writes_little_endian_float64() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.0).unwrap();
        let (mut a0, mut a1) = (Operand::idx(Reg::S(0), 0), Operand::r(Reg::F(0)));
        op_flt2y8(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(
            vm.regs.string(Reg::S(0)).unwrap().get(false),
            vec![0, 0, 0, 0, 0, 0, 0xF0, 0x3F]
        );
    }

    #[test]
    fn y42flt_reads_little_endian_float32() {
        let mut vm = vm();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm_str(&[0, 0, 0x80, 0x3F]));
        op_y42flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 1.0);
    }

    #[test]
    fn y82flt_reads_little_endian_float64() {
        let mut vm = vm();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm_str(&2.5f64.to_le_bytes()));
        op_y82flt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 2.5);

        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::imm_str(&[0, 0, 0]));
        assert!(matches!(
            op_y82flt(&mut vm, oc(), &mut a0, &mut a1),
            Err(VmError::Internal(_))
        ));
    }

    #[test]
    fn fcomp_sets_flags() {
        let mut vm = vm();
        vm.regs.set_float(Reg::F(0), 1.0).unwrap();
        vm.regs.set_float(Reg::F(1), 2.0).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::F(0)), Operand::r(Reg::F(1)));
        op_fcomp(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.sign);
        assert!(!vm.flags.zero);
        assert!(!vm.flags.carry);

        vm.regs.set_float(Reg::F(1), 1.0).unwrap();
        op_fcomp(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.zero);
        assert!(!vm.flags.sign);

        vm.regs.set_float(Reg::F(0), f64::INFINITY).unwrap();
        vm.regs.set_float(Reg::F(1), f64::INFINITY).unwrap();
        op_fcomp(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.carry);
    }

    #[test]
    fn parse_double_invariant_grammar() {
        assert_eq!(parse_double_invariant("  -1.5e3 "), Some(-1500.0));
        assert_eq!(parse_double_invariant(".5"), Some(0.5));
        assert_eq!(parse_double_invariant("5."), Some(5.0));
        assert_eq!(parse_double_invariant(""), None);
        assert_eq!(parse_double_invariant("."), None);
        assert_eq!(parse_double_invariant("1e"), None);
        assert_eq!(parse_double_invariant("1 2"), None);
        assert_eq!(parse_double_invariant("-Infinity"), Some(f64::NEG_INFINITY));
    }
}
