//! Opcodes: a2fix, fix2hex, fix2dez, a2y, hex2y, y2bcd, y2hex, ufix2dez.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpA2Fix` (a2fix)
pub(crate) fn op_a2fix(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpA2fix: Invalid type");
    }
    let s = vm.op_string(arg1)?;
    let mut value = string_to_value(&s);
    if value < i32::MIN as i64 {
        value = i32::MIN as i64;
    }
    // Quirk: values above Int32.MaxValue become 0xFFFFFFFF, not Int32.MaxValue.
    if value > i32::MAX as i64 {
        value = 0xFFFF_FFFF;
    }
    vm.op_set_raw(arg0, Value::Int(value as u32), 1)?;
    vm.flags.zero = false;
    vm.flags.sign = false;
    vm.flags.overflow = false;
    Ok(())
}

/// C#: `OpFix2Hex` (fix2hex)
pub(crate) fn op_fix2hex(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFix2hex: Invalid type");
    }
    let len = value_len(vm, arg1)?;
    let value = vm.op_value(arg1, len)?;
    let result = match len {
        1 => format!("0x{:02X}", value),
        2 => format!("0x{:04X}", value),
        4 => format!("0x{:08X}", value),
        _ => return internal("OpFix2hex: Invalid length"),
    };
    vm.op_set_string(arg0, &result)
}

/// C#: `OpFix2Dez` (fix2dez)
pub(crate) fn op_fix2dez(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFix2dez: Invalid type");
    }
    let len = value_len(vm, arg1)?;
    let value = vm.op_value(arg1, len)?;
    let result = match len {
        1 => format!("{}", value as u8 as i8),
        2 => format!("{}", value as u16 as i16),
        4 => format!("{}", value as i32),
        _ => return internal("OpFix2dez: Invalid length"),
    };
    vm.op_set_string(arg0, &result)
}

/// C#: `OpA2Y` (a2y)
pub(crate) fn op_a2y(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpA2y: Invalid type");
    }
    let mut string_data = vm.op_string(arg1)?;
    let mut result: Vec<u8> = Vec::new();
    if !string_data.is_empty() {
        // Cut the string at the first character that is not a hex digit,
        // space, ';' or ','. Case insensitive, as the C# lower-cases first.
        let cut = string_data.char_indices().find(|&(_, c)| {
            let c = c.to_ascii_lowercase();
            !(c.is_ascii_digit() || ('a'..='f').contains(&c) || c == ' ' || c == ';' || c == ',')
        });
        if let Some((pos, _)) = cut {
            string_data.truncate(pos);
        }

        // After the cut the string is ASCII only.
        'outer: for string_value in string_data.split([',', ';']) {
            let trimmed = string_value.trim();
            if trimmed.is_empty() {
                // Quirk: each empty field yields Length + 1 zero bytes.
                result.resize(result.len() + string_value.len() + 1, 0);
            } else {
                for sub in trimmed.split(' ') {
                    if sub.is_empty() {
                        continue;
                    }
                    match byte_from_hex(sub) {
                        Some(b) => result.push(b),
                        // C#: exception in Convert.ToByte stops all parsing.
                        None => break 'outer,
                    }
                }
            }
        }
    }
    vm.op_set_array(arg0, result)
}

/// C#: `OpHex2Y` (hex2y)
pub(crate) fn op_hex2y(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpHex2y: Invalid type");
    }
    let value_str = vm.op_string(arg1)?;
    // C# quirk: HexToByteArray swallows its own exceptions and returns an
    // empty array, so the catch branch below is never taken and Carry is
    // always cleared.
    let result = hex_to_byte_array(&value_str);
    vm.flags.carry = false;
    vm.op_set_array(arg0, result)
}

/// C#: `OpY2Bcd` (y2bcd)
pub(crate) fn op_y2bcd(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpY2bcd: Invalid type");
    }
    let data = vm.op_array(arg1)?;
    let mut result = String::new();
    for b in data {
        result.push_str(&value_to_bcd(b));
    }
    vm.op_set_string(arg0, &result)
}

/// C#: `OpY2Hex` (y2hex)
pub(crate) fn op_y2hex(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpY2hex: Invalid type");
    }
    let data = vm.op_array(arg1)?;
    let mut result = String::new();
    for b in data {
        result.push_str(&format!("{:02X}", b));
    }
    vm.op_set_string(arg0, &result)
}

/// C#: `OpUfix2Dez` (ufix2dez)
pub(crate) fn op_ufix2dez(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpUfix2dez: Invalid type");
    }
    let len = value_len(vm, arg1)?;
    let value = vm.op_value(arg1, len)?;
    let result = match len {
        1 => format!("{}", value as u8),
        2 => format!("{}", value as u16),
        4 => format!("{}", value),
        _ => return internal("OpUfix2dez: Invalid length"),
    };
    vm.op_set_string(arg0, &result)
}

/// C#: `arg1.GetDataType() != typeof(EdValueType) ? 1 : arg1.GetDataLen()`.
fn value_len(vm: &mut Vm, arg1: &Operand) -> VmResult<u32> {
    if arg1.data_type() != DataType::Value {
        Ok(1)
    } else {
        vm.op_data_len(arg1, false)
    }
}

/// C#: `EdiabasNet.StringToValue(string)` (value part only; the `valid`
/// out parameter is not needed by the handlers ported here).
/// Hex `0x`, binary `0y`, decimal. Any parse failure gives 0.
fn string_to_value(number: &str) -> i64 {
    let local = number.trim_end();
    if local.is_empty() {
        return 0;
    }
    let lower = local.to_lowercase();
    if lower.starts_with("0x") {
        if lower.chars().count() > 2 {
            let first = lower.chars().nth(2).unwrap_or('\0');
            if first.is_ascii_digit() || ('a'..='f').contains(&first) {
                let mut body: &str = &local[2..];
                // Convert.ToInt64(s, 16) accepts an optional 0x prefix.
                if body.len() >= 2 && body[..2].eq_ignore_ascii_case("0x") {
                    body = &body[2..];
                }
                return parse_unsigned_radix(body, 16).map(|v| v as i64).unwrap_or(0);
            }
        }
        return 0;
    }
    if lower.starts_with("0y") {
        return parse_unsigned_radix(&local[2..], 2).map(|v| v as i64).unwrap_or(0);
    }
    if lower != "-" && lower != "--" && lower.chars().next().is_some_and(|c| !c.is_alphabetic()) {
        let conv = local.trim_start();
        let conv = match conv.find(['.', ',']) {
            Some(i) => &conv[..i],
            None => conv,
        };
        return parse_decimal_i64(conv).unwrap_or(0);
    }
    0
}

/// C#: `Convert.ToInt64(s, radix)` for radix 2 or 16. Two's complement
/// wrap is allowed up to the 64 bit range; larger values fail.
fn parse_unsigned_radix(s: &str, radix: u32) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let mut v: u64 = 0;
    for c in s.chars() {
        let d = c.to_digit(radix)? as u64;
        v = v.checked_mul(radix as u64)?.checked_add(d)?;
    }
    Some(v)
}

/// C#: `Convert.ToInt64(s, 10)`: optional sign, digits, i64 range.
fn parse_decimal_i64(s: &str) -> Option<i64> {
    let (neg, digits) = match s.as_bytes().first()? {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut v: i128 = 0;
    for b in digits.bytes() {
        v = v * 10 + (b - b'0') as i128;
        if v > (i64::MAX as i128) + 1 {
            return None;
        }
    }
    if neg {
        v = -v;
    }
    i64::try_from(v).ok()
}

/// C#: `Convert.ToByte(s, 16)`: hex digits, value must fit in a byte.
fn byte_from_hex(s: &str) -> Option<u8> {
    // from_str_radix would also accept a leading '+'; C# does not.
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    u8::try_from(v).ok()
}

/// C#: `EdiabasNet.HexToByteArray`. Pairs of hex digits; an odd trailing
/// char is ignored. Any bad pair makes the whole result empty.
fn hex_to_byte_array(value_str: &str) -> Vec<u8> {
    let chars: Vec<char> = value_str.chars().collect();
    let length = chars.len() - chars.len() % 2;
    let mut result = Vec::with_capacity(length / 2);
    for pair in chars[..length].chunks(2) {
        let text: String = pair.iter().collect();
        match byte_from_hex(&text) {
            Some(b) => result.push(b),
            None => return Vec::new(),
        }
    }
    result
}

/// C#: `EdiabasNet.ValueToBcd`. A nibble above 9 becomes '*'.
fn value_to_bcd(value: u8) -> String {
    let mut result = String::new();
    let part1 = (value >> 4) & 0x0F;
    let part2 = value & 0x0F;
    if part1 > 9 {
        result.push('*');
    } else {
        result.push_str(&format!("{:X}", part1));
    }
    if part2 > 9 {
        result.push('*');
    } else {
        result.push_str(&format!("{:X}", part2));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regs::Reg;

    fn oc() -> OpCode {
        OpCode { code: 0, name: "test", func: None, arg0_is_near_address: false }
    }

    fn out_str(vm: &mut Vm, a0: &Operand) -> String {
        vm.op_string(a0).unwrap()
    }

    #[test]
    fn a2fix_parses_negative_and_clamps() {
        let mut vm = Vm::for_test();
        vm.flags.zero = true;
        vm.flags.sign = true;
        vm.flags.overflow = true;
        vm.flags.carry = true;
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm_str(b"-1"));
        op_a2fix(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0xFFFF_FFFF);
        assert!(!vm.flags.zero && !vm.flags.sign && !vm.flags.overflow);
        assert!(vm.flags.carry);

        let mut a1 = Operand::imm_str(b"0x10");
        op_a2fix(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 16);

        let mut a1 = Operand::imm_str(b"9999999999");
        op_a2fix(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0xFFFF_FFFF);

        let mut a1 = Operand::imm_str(b"-9999999999");
        op_a2fix(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0x8000_0000);
    }

    #[test]
    fn fix2hex_formats_by_length() {
        let mut vm = Vm::for_test();
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm8(0x5A);
        op_fix2hex(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "0x5A");

        let mut a1 = Operand::imm16(0x0ABC);
        op_fix2hex(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "0x0ABC");

        let mut a1 = Operand::imm32(0xDEAD_BEEF);
        op_fix2hex(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "0xDEADBEEF");
    }

    #[test]
    fn fix2dez_signed_including_negative() {
        let mut vm = Vm::for_test();
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm8(0xFF);
        op_fix2dez(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "-1");

        let mut a1 = Operand::imm16(0xFFFE);
        op_fix2dez(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "-2");

        let mut a1 = Operand::imm32(0x8000_0000);
        op_fix2dez(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "-2147483648");

        let mut a1 = Operand::imm32(42);
        op_fix2dez(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "42");
    }

    #[test]
    fn a2y_parses_hex_bytes_and_empty_fields() {
        let mut vm = Vm::for_test();
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm_str(b"01 0a;ff,,  ");
        op_a2y(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x01, 0x0A, 0xFF, 0, 0, 0, 0]);

        // Invalid char cuts the string; "zz" and after are ignored.
        let mut a1 = Operand::imm_str(b"12 zz 34");
        op_a2y(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x12]);

        // Value above a byte stops parsing with an empty result.
        let mut a1 = Operand::imm_str(b"1ff");
        op_a2y(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn hex2y_converts_and_clears_carry() {
        let mut vm = Vm::for_test();
        vm.flags.carry = true;
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm_str(b"0a1B");
        op_hex2y(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x0A, 0x1B]);
        assert!(!vm.flags.carry);

        // Odd trailing char is dropped.
        let mut a1 = Operand::imm_str(b"0a1");
        op_hex2y(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), vec![0x0A]);

        // Bad pair: empty result, and Carry is still false (C# quirk).
        vm.flags.carry = true;
        let mut a1 = Operand::imm_str(b"zz");
        op_hex2y(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.op_array(&a0).unwrap(), Vec::<u8>::new());
        assert!(!vm.flags.carry);
    }

    #[test]
    fn y2bcd_maps_nibbles() {
        let mut vm = Vm::for_test();
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm_str(&[0x12, 0xAB, 0x0F]);
        op_y2bcd(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "12**0*");
    }

    #[test]
    fn y2hex_uppercase_two_digits() {
        let mut vm = Vm::for_test();
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm_str(&[0x0A, 0xFF, 0x00]);
        op_y2hex(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "0AFF00");
    }

    #[test]
    fn ufix2dez_unsigned() {
        let mut vm = Vm::for_test();
        let mut a0 = Operand::r(Reg::S(0));
        let mut a1 = Operand::imm8(0xFF);
        op_ufix2dez(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "255");

        let mut a1 = Operand::imm32(0xFFFF_FFFF);
        op_ufix2dez(&mut vm, &oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(out_str(&mut vm, &a0), "4294967295");
    }
}
