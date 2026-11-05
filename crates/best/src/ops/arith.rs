//! Opcodes: move, clear, comp, subb, adds, mult, divs, and, or, xor, not, clrc, setc, asr, lsl, lsr, asl, addc, subc, clrv, test.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::regs::Reg;
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `GetArgsValueLength(arg0, arg1)` = `arg0.GetDataLen(true)`.
fn args_len(vm: &mut Vm, arg0: &Operand) -> VmResult<u32> {
    vm.op_data_len(arg0, true)
}

/// C#: `Flags.UpdateFlags` / `SetOverflow` / `SetCarry` throw `ArgumentOutOfRangeException`
/// for any length other than 1, 2 or 4. The Rust flags helpers panic, so check first.
fn flags_len(len: u32) -> VmResult<u32> {
    match len {
        1 | 2 | 4 => Ok(len),
        _ => internal(format!("Flags: invalid length {len}")),
    }
}

/// Operand length for flag-setting arithmetic, validated for the flag helpers.
fn arith_len(vm: &mut Vm, arg0: &Operand) -> VmResult<u32> {
    let len = args_len(vm, arg0)?;
    flags_len(len)
}

/// C#: `OpMove` (move)
pub(crate) fn op_move(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpMove: Invalid type");
    }
    let data0 = arg0.data_type();
    let data1 = arg1.data_type();
    match data0 {
        DataType::Value => {
            let len = args_len(vm, arg0)?;
            let value = vm.op_value(arg1, len)?;
            vm.op_set_raw(arg0, Value::Int(value), 1)?;
            vm.flags.carry = false;
            vm.flags.overflow = false;
            vm.flags.update_flags(value, flags_len(len)?);
            Ok(())
        }
        DataType::Array => match data1 {
            DataType::Value => {
                let len = args_len(vm, arg0)?;
                let value = vm.op_value(arg1, len)?;
                // C# quirk: SetRawData(uint) on an S register throws (Register.SetRawData
                // needs byte[]), so this branch only works for indexed targets like S[i].
                vm.op_set_raw(arg0, Value::Int(value), 1)?;
                vm.flags.carry = false;
                vm.flags.overflow = false;
                vm.flags.update_flags(value, 1);
                Ok(())
            }
            DataType::Array => {
                let source = vm.op_raw(arg1)?;
                if let (AddrMode::RegS, Value::Bytes(src)) = (arg0.mode, &source) {
                    let mut dest = vm.op_array(arg0)?;
                    if dest.len() < src.len() {
                        dest.resize(src.len(), 0);
                    }
                    dest[..src.len()].copy_from_slice(src);
                    vm.op_set_raw(arg0, Value::Bytes(dest), 1)?;
                } else {
                    // C# calls arg1.GetRawData() a second time in this branch.
                    let raw = vm.op_raw(arg1)?;
                    vm.op_set_raw(arg0, raw, 1)?;
                }
                vm.flags.carry = false;
                vm.flags.zero = false;
                vm.flags.sign = false;
                vm.flags.overflow = false;
                Ok(())
            }
            DataType::Float => internal("OpMove: Invalid source data type"),
        },
        DataType::Float => internal("OpMove: Invalid target data type"),
    }
}

/// C#: `OpClear` (clear)
pub(crate) fn op_clear(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let Some(r) = arg0.reg() else {
        return internal("OpClear: Invalid type");
    };
    // C# dispatches on the register type (byte[] / float / value), not on the address mode.
    match r {
        Reg::S(_) => vm.regs.string_mut(r)?.clear(),
        Reg::F(_) => vm.regs.set_float(r, 0.0)?,
        _ => vm.regs.set_value(r, 0)?,
    }
    vm.flags.carry = false;
    vm.flags.zero = true;
    vm.flags.sign = false;
    vm.flags.overflow = false;
    Ok(())
}

/// C#: `OpComp` (comp)
pub(crate) fn op_comp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let len = arith_len(vm, arg0)?;
    let val0 = vm.op_value(arg0, len)?;
    let val1 = vm.op_value(arg1, len)?;

    let diff = (val0 as u64).wrapping_sub(val1 as u64);
    let diff32 = diff as u32;
    vm.flags.update_flags(diff32, len);
    vm.flags.set_overflow(val0, 0u32.wrapping_sub(val1), diff32, len);
    vm.flags.set_carry(diff, len);
    Ok(())
}

/// C#: `OpSubb` (subb)
pub(crate) fn op_subb(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpSubb: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let val0 = vm.op_value(arg0, len)?;
    let val1 = vm.op_value(arg1, len)?;

    let diff = (val0 as u64).wrapping_sub(val1 as u64);
    let diff32 = diff as u32;
    vm.op_set_raw(arg0, Value::Int(diff32), 1)?;
    vm.flags.update_flags(diff32, len);
    vm.flags.set_overflow(val0, 0u32.wrapping_sub(val1), diff32, len);
    vm.flags.set_carry(diff, len);
    Ok(())
}

/// C#: `OpAdds` (adds)
pub(crate) fn op_adds(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpAdds: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let val0 = vm.op_value(arg0, len)?;
    let val1 = vm.op_value(arg1, len)?;

    let sum = val0 as u64 + val1 as u64;
    let sum32 = sum as u32;
    vm.op_set_raw(arg0, Value::Int(sum32), 1)?;
    vm.flags.update_flags(sum32, len);
    vm.flags.set_overflow(val0, val1, sum32, len);
    vm.flags.set_carry(sum, len);
    Ok(())
}

/// C#: `OpMult` (mult)
pub(crate) fn op_mult(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpMult: Invalid type");
    }
    // C#: an invalid length throws inside the try and is rethrown as "OpMult mult failure".
    let len = arith_len(vm, arg0)?;
    let value1 = vm.op_value(arg0, len)?;
    let value2 = vm.op_value(arg1, len)?;
    let result: u32 = match len {
        1 => (value1 as u8 as i8 as i32).wrapping_mul(value2 as u8 as i8 as i32) as u32,
        2 => (value1 as u16 as i16 as i32).wrapping_mul(value2 as u16 as i16 as i32) as u32,
        _ => (value1 as i32).wrapping_mul(value2 as i32) as u32,
    };
    vm.op_set_raw(arg0, Value::Int(result), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(result, len);

    if arg1.reg().is_some() {
        // High part of the wide product goes to arg1.
        let high = ((result as u64) >> (len << 3)) as u32;
        vm.op_set_raw(arg1, Value::Int(high), len)?;
    }
    Ok(())
}

/// C#: `OpDivs` (divs)
pub(crate) fn op_divs(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpDivs: Invalid type");
    }
    let len = args_len(vm, arg0)?;
    let value1 = vm.op_value(arg0, len)?;
    let value2 = vm.op_value(arg1, len)?;

    // C# does Int32 math for every length. Divide by zero, Int32.MinValue / -1 and an
    // invalid length all throw inside the try. The catch calls RaiseError(BIP_0007).
    // IsMinVersion760 is always true (EdiabasVersion = 0x773), so this is never a
    // trapped SetError. RaiseError always throws, so the C# error-continue path is dead.
    let (a, b) = (value1 as i32, value2 as i32);
    let (result, remainder) = match (len, a.checked_div(b), a.checked_rem(b)) {
        (1 | 2 | 4, Some(q), Some(r)) => (q as u32, r as u32),
        _ => return Err(vm.raise_error(ErrorCode::EDIABAS_BIP_0007)),
    };
    vm.flags.overflow = false;
    vm.flags.update_flags(result, len);
    vm.op_set_raw(arg0, Value::Int(result), 1)?;
    if arg1.reg().is_some() {
        vm.op_set_raw(arg1, Value::Int(remainder), len)?;
    }
    Ok(())
}

/// C#: `OpAnd` (and)
pub(crate) fn op_and(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpAnd: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let value = vm.op_value(arg0, len)? & vm.op_value(arg1, len)?;
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpOr` (or)
pub(crate) fn op_or(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpOr: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let value = vm.op_value(arg0, len)? | vm.op_value(arg1, len)?;
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpXor` (xor)
pub(crate) fn op_xor(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpXor: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let value = vm.op_value(arg0, len)? ^ vm.op_value(arg1, len)?;
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpNot` (not)
pub(crate) fn op_not(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpNot: Invalid type");
    }
    let value = !vm.op_value(arg0, 0)?;
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    // C# re-reads the operand here, so flags come from the value as stored.
    let stored = vm.op_value(arg0, 0)?;
    let len = vm.op_data_len(arg0, false)?;
    vm.flags.update_flags(stored, flags_len(len)?);
    Ok(())
}

/// C#: `OpClrc` (clrc)
pub(crate) fn op_clrc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    vm.flags.carry = false;
    Ok(())
}

/// C#: `OpSetc` (setc)
pub(crate) fn op_setc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    vm.flags.carry = true;
    Ok(())
}

/// C#: `OpAsr` (asr)
pub(crate) fn op_asr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpAsr: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let bits = len * 8;
    let mut value = vm.op_value(arg0, len)?;
    let shift = vm.op_value(arg1, len)? as i32;
    if shift < 0 {
        // C#: don't touch carry here!
    } else if shift == 0 {
        vm.flags.carry = false;
    } else {
        if shift as u32 > bits {
            let sign_mask = 1u32 << (bits - 1);
            vm.flags.carry = value & sign_mask != 0;
        } else {
            let carry_mask = 1u32 << (shift as u32 - 1);
            vm.flags.carry = value & carry_mask != 0;
        }

        if shift as u32 >= bits {
            let sign_mask = 1u32 << (bits - 1);
            value = if value & sign_mask != 0 { 0xFFFF_FFFF } else { 0 };
        } else {
            // C#: SByte / Int16 / Int32 arithmetic shift, widened back to uint.
            value = match len {
                1 => ((value as u8 as i8 as i32) >> shift) as u32,
                2 => ((value as u16 as i16 as i32) >> shift) as u32,
                _ => ((value as i32) >> shift) as u32,
            };
        }
    }
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// Shared body of C# `OpLsl` and `OpAsl` (identical in the C# source).
fn shift_left(vm: &mut Vm, arg0: &mut Operand, arg1: &mut Operand, name: &str) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal(format!("{name}: Invalid type"));
    }
    let len = arith_len(vm, arg0)?;
    let bits = len * 8;
    let mut value = vm.op_value(arg0, len)?;
    let shift = vm.op_value(arg1, len)? as i32;
    if shift < 0 {
        // C#: don't touch carry here!
    } else if shift == 0 {
        vm.flags.carry = false;
    } else {
        if shift as u32 > bits {
            vm.flags.carry = false;
        } else {
            let carry_shift = bits - shift as u32;
            vm.flags.carry = value & (1u32 << carry_shift) != 0;
        }

        if shift as u32 >= bits {
            value = 0;
        } else {
            value <<= shift as u32;
        }
    }
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpLsl` (lsl)
pub(crate) fn op_lsl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    shift_left(vm, arg0, arg1, "OpLsl")
}

/// C#: `OpLsr` (lsr)
pub(crate) fn op_lsr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpLsr: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let bits = len * 8;
    let mut value = vm.op_value(arg0, len)?;
    let shift = vm.op_value(arg1, len)? as i32;
    if shift < 0 {
        // C#: don't touch carry here!
    } else if shift == 0 {
        vm.flags.carry = false;
    } else {
        if shift as u32 > bits {
            vm.flags.carry = false;
        } else {
            let carry_shift = shift as u32 - 1;
            vm.flags.carry = value & (1u32 << carry_shift) != 0;
        }

        if shift as u32 >= bits {
            value = 0;
        } else {
            value >>= shift as u32;
        }
    }
    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpAsl` (asl)
pub(crate) fn op_asl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    shift_left(vm, arg0, arg1, "OpAsl")
}

/// C#: `OpAddc` (addc)
pub(crate) fn op_addc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpAddc: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let val0 = vm.op_value(arg0, len)?;
    let mut val1 = vm.op_value(arg1, len)?;
    if vm.flags.carry {
        // C# uint++ wraps.
        val1 = val1.wrapping_add(1);
    }

    let sum = val0 as u64 + val1 as u64;
    let sum32 = sum as u32;
    vm.op_set_raw(arg0, Value::Int(sum32), 1)?;
    vm.flags.update_flags(sum32, len);
    vm.flags.set_overflow(val0, val1, sum32, len);
    vm.flags.set_carry(sum, len);
    Ok(())
}

/// C#: `OpSubc` (subc)
pub(crate) fn op_subc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpSubc: Invalid type");
    }
    let len = arith_len(vm, arg0)?;
    let val0 = vm.op_value(arg0, len)?;
    let mut val1 = vm.op_value(arg1, len)?;
    if vm.flags.carry {
        // C# uint++ wraps.
        val1 = val1.wrapping_add(1);
    }

    let diff = (val0 as u64).wrapping_sub(val1 as u64);
    let diff32 = diff as u32;
    vm.op_set_raw(arg0, Value::Int(diff32), 1)?;
    vm.flags.update_flags(diff32, len);
    vm.flags.set_overflow(val0, 0u32.wrapping_sub(val1), diff32, len);
    vm.flags.set_carry(diff, len);
    Ok(())
}

/// C#: `OpClrv` (clrv)
pub(crate) fn op_clrv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    vm.flags.overflow = false;
    Ok(())
}

/// C#: `OpTest` (test)
pub(crate) fn op_test(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let len = arith_len(vm, arg0)?;
    let value = vm.op_value(arg0, len)? & vm.op_value(arg1, len)?;
    vm.flags.overflow = false;
    vm.flags.update_flags(value, len);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::OPCODES;

    fn oc() -> &'static OpCode {
        &OPCODES[0]
    }

    fn vm() -> Vm {
        let mut vm = Vm::for_test();
        vm.set_array_max_buf_size(64);
        vm
    }

    fn l(i: u8) -> Operand {
        Operand::r(Reg::L(i))
    }

    fn b(i: u8) -> Operand {
        Operand::r(Reg::B(i))
    }

    fn lv(vm: &Vm, i: u8) -> u32 {
        vm.regs.value(Reg::L(i)).unwrap()
    }

    fn bv(vm: &Vm, i: u8) -> u32 {
        vm.regs.value(Reg::B(i)).unwrap()
    }

    fn set_l(vm: &mut Vm, i: u8, v: u32) {
        vm.regs.set_value(Reg::L(i), v).unwrap();
    }

    fn set_b(vm: &mut Vm, i: u8, v: u32) {
        vm.regs.set_value(Reg::B(i), v).unwrap();
    }

    #[test]
    fn move_value_to_reg_sets_flags() {
        let mut vm = vm();
        vm.flags.carry = true;
        vm.flags.overflow = true;
        let (mut a0, mut a1) = (l(0), Operand::imm32(0x8000_0000));
        op_move(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(lv(&vm, 0), 0x8000_0000);
        assert!(!vm.flags.carry && !vm.flags.overflow);
        assert!(vm.flags.sign && !vm.flags.zero);
    }

    #[test]
    fn move_zero_value_sets_zero_flag() {
        let mut vm = vm();
        let (mut a0, mut a1) = (b(1), Operand::imm8(0));
        op_move(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.zero && !vm.flags.sign);
    }

    #[test]
    fn move_value_to_string_reg_is_error() {
        let mut vm = vm();
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm8(1));
        let r = op_move(&mut vm, oc(), &mut a0, &mut a1);
        assert!(matches!(r, Err(VmError::Internal(_))));
    }

    #[test]
    fn move_array_from_array_copies_prefix() {
        let mut vm = vm();
        vm.reg_set_array(Reg::S(0), b"a", false).unwrap();
        vm.reg_set_array(Reg::S(1), b"xyz", false).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::r(Reg::S(1)));
        vm.flags.carry = true;
        vm.flags.zero = true;
        op_move(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false), b"xyz".to_vec());
        assert!(!vm.flags.carry && !vm.flags.zero && !vm.flags.sign && !vm.flags.overflow);
    }

    #[test]
    fn move_array_from_value_into_index_writes_byte() {
        let mut vm = vm();
        vm.reg_set_array(Reg::S(0), b"abc", false).unwrap();
        let (mut a0, mut a1) = (Operand::idx(Reg::S(0), 1), Operand::imm8(0x5A));
        op_move(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false), b"aZc".to_vec());
        assert!(!vm.flags.carry && !vm.flags.overflow);
    }

    #[test]
    fn clear_reg_resets_value_and_flags() {
        let mut vm = vm();
        set_l(&mut vm, 0, 5);
        vm.flags.carry = true;
        vm.flags.sign = true;
        vm.flags.overflow = true;
        op_clear(&mut vm, oc(), &mut l(0), &mut Operand::none()).unwrap();
        assert_eq!(lv(&vm, 0), 0);
        assert!(!vm.flags.carry && vm.flags.zero && !vm.flags.sign && !vm.flags.overflow);
    }

    #[test]
    fn clear_string_reg_empties_it() {
        let mut vm = vm();
        vm.reg_set_array(Reg::S(0), b"abc", false).unwrap();
        op_clear(&mut vm, oc(), &mut Operand::r(Reg::S(0)), &mut Operand::none()).unwrap();
        assert!(vm.regs.string(Reg::S(0)).unwrap().get(false).is_empty());
    }

    #[test]
    fn comp_equal_sets_zero_clears_carry() {
        let mut vm = vm();
        set_l(&mut vm, 0, 5);
        set_l(&mut vm, 1, 5);
        vm.flags.carry = true;
        op_comp(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert!(vm.flags.zero && !vm.flags.carry && !vm.flags.sign && !vm.flags.overflow);
        assert_eq!(lv(&vm, 0), 5, "comp does not write arg0");
    }

    #[test]
    fn comp_less_sets_carry_and_sign() {
        let mut vm = vm();
        set_l(&mut vm, 0, 1);
        set_l(&mut vm, 1, 2);
        op_comp(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert!(vm.flags.carry && vm.flags.sign && !vm.flags.zero && !vm.flags.overflow);
    }

    #[test]
    fn comp_signed_overflow() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0x8000_0000);
        set_l(&mut vm, 1, 1);
        op_comp(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert!(vm.flags.overflow && !vm.flags.carry);
        assert_eq!(vm.flags.sign, false);
    }

    #[test]
    fn subb_basic_no_flags() {
        let mut vm = vm();
        set_l(&mut vm, 0, 5);
        set_l(&mut vm, 1, 3);
        op_subb(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 2);
        assert!(!vm.flags.carry && !vm.flags.zero && !vm.flags.sign && !vm.flags.overflow);
    }

    #[test]
    fn subb_borrow_sets_carry_and_sign() {
        let mut vm = vm();
        set_l(&mut vm, 0, 3);
        set_l(&mut vm, 1, 5);
        op_subb(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0xFFFF_FFFE);
        assert!(vm.flags.carry && vm.flags.sign);
    }

    #[test]
    fn subb_signed_overflow() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0x8000_0000);
        set_l(&mut vm, 1, 1);
        op_subb(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0x7FFF_FFFF);
        assert!(vm.flags.overflow && !vm.flags.carry && !vm.flags.sign);
    }

    #[test]
    fn subb_equal_sets_zero() {
        let mut vm = vm();
        set_b(&mut vm, 0, 4);
        set_b(&mut vm, 1, 4);
        op_subb(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert!(vm.flags.zero && !vm.flags.carry);
    }

    #[test]
    fn adds_basic() {
        let mut vm = vm();
        set_l(&mut vm, 0, 2);
        set_l(&mut vm, 1, 3);
        op_adds(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 5);
        assert!(!vm.flags.carry && !vm.flags.zero && !vm.flags.overflow);
    }

    #[test]
    fn adds_carry_out_sets_carry_and_zero() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0xFFFF_FFFF);
        set_l(&mut vm, 1, 1);
        op_adds(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0);
        assert!(vm.flags.carry && vm.flags.zero && !vm.flags.overflow);
    }

    #[test]
    fn adds_signed_overflow() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0x7FFF_FFFF);
        set_l(&mut vm, 1, 1);
        op_adds(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0x8000_0000);
        assert!(vm.flags.overflow && vm.flags.sign && !vm.flags.carry);
    }

    #[test]
    fn adds_byte_carry_uses_bit_8() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0xFF);
        set_b(&mut vm, 1, 0x01);
        op_adds(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0);
        assert!(vm.flags.carry && vm.flags.zero);
    }

    #[test]
    fn mult_word_signed() {
        let mut vm = vm();
        set_l(&mut vm, 0, 3);
        set_l(&mut vm, 1, (-2i32) as u32);
        op_mult(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), (-6i32) as u32);
        assert_eq!(lv(&vm, 1), 0, "high part of 32 bit product is 0 here");
        assert!(vm.flags.sign && !vm.flags.overflow);
    }

    #[test]
    fn mult_byte_writes_high_part_to_arg1() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x10);
        set_b(&mut vm, 1, 0x10);
        op_mult(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0);
        assert_eq!(bv(&vm, 1), 1);
        assert!(vm.flags.zero);
    }

    #[test]
    fn divs_basic_sets_quotient_and_remainder() {
        let mut vm = vm();
        set_l(&mut vm, 0, 7);
        set_l(&mut vm, 1, 2);
        op_divs(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 3);
        assert_eq!(lv(&vm, 1), 1);
        assert!(!vm.flags.zero && !vm.flags.overflow);
    }

    #[test]
    fn divs_negative_is_signed() {
        let mut vm = vm();
        set_l(&mut vm, 0, (-7i32) as u32);
        set_l(&mut vm, 1, 2);
        op_divs(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), (-3i32) as u32);
        assert_eq!(lv(&vm, 1), (-1i32) as u32);
        assert!(vm.flags.sign);
    }

    #[test]
    fn divs_by_zero_raises_bip_0007() {
        let mut vm = vm();
        set_l(&mut vm, 0, 7);
        set_l(&mut vm, 1, 0);
        let r = op_divs(&mut vm, oc(), &mut l(0), &mut l(1));
        assert!(matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0007))));
    }

    #[test]
    fn divs_min_by_minus_one_raises_bip_0007() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0x8000_0000);
        set_l(&mut vm, 1, (-1i32) as u32);
        let r = op_divs(&mut vm, oc(), &mut l(0), &mut l(1));
        assert!(matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0007))));
    }

    #[test]
    fn and_basic_clears_overflow() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0xF0F0);
        set_l(&mut vm, 1, 0x0FF0);
        vm.flags.overflow = true;
        op_and(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0x00F0);
        assert!(!vm.flags.overflow && !vm.flags.zero && !vm.flags.sign);
    }

    #[test]
    fn and_zero_result_sets_zero() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0xF0);
        set_l(&mut vm, 1, 0x0F);
        op_and(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert!(vm.flags.zero);
    }

    #[test]
    fn or_byte_sets_sign() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x0F);
        set_b(&mut vm, 1, 0xF0);
        op_or(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0xFF);
        assert!(vm.flags.sign && !vm.flags.zero);
    }

    #[test]
    fn xor_equal_gives_zero() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0xFF);
        set_l(&mut vm, 1, 0xFF);
        op_xor(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0);
        assert!(vm.flags.zero);
    }

    #[test]
    fn not_byte_masks_to_width_and_sets_sign() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x0F);
        vm.flags.overflow = true;
        op_not(&mut vm, oc(), &mut b(0), &mut Operand::none()).unwrap();
        assert_eq!(bv(&vm, 0), 0xF0);
        assert!(vm.flags.sign && !vm.flags.zero && !vm.flags.overflow);
    }

    #[test]
    fn clrc_setc_clrv_touch_only_their_flag() {
        let mut vm = vm();
        vm.flags.zero = true;
        op_setc(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert!(vm.flags.carry && vm.flags.zero);
        op_clrc(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert!(!vm.flags.carry && vm.flags.zero);
        vm.flags.overflow = true;
        op_clrv(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert!(!vm.flags.overflow && vm.flags.zero);
    }

    #[test]
    fn asr_word_keeps_sign() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0x8000_0000);
        set_l(&mut vm, 1, 4);
        op_asr(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0xF800_0000);
        assert!(vm.flags.sign && !vm.flags.carry);
    }

    #[test]
    fn asr_byte_carry_from_last_shifted_bit() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x81);
        set_b(&mut vm, 1, 1);
        op_asr(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0xC0);
        assert!(vm.flags.carry && vm.flags.sign);
    }

    #[test]
    fn asr_shift_past_width_fills_with_sign() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0x8000_0000);
        set_l(&mut vm, 1, 40);
        op_asr(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0xFFFF_FFFF);
        assert!(vm.flags.carry && vm.flags.sign);
    }

    #[test]
    fn lsl_byte_carry_from_top_bit() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x81);
        set_b(&mut vm, 1, 1);
        op_lsl(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0x02);
        assert!(vm.flags.carry && !vm.flags.zero && !vm.flags.sign);
    }

    #[test]
    fn lsl_zero_shift_clears_carry() {
        let mut vm = vm();
        set_b(&mut vm, 0, 5);
        set_b(&mut vm, 1, 0);
        vm.flags.carry = true;
        op_lsl(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 5);
        assert!(!vm.flags.carry);
    }

    #[test]
    fn lsl_shift_past_width_gives_zero_no_carry() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0xFF);
        set_b(&mut vm, 1, 9);
        op_lsl(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0);
        assert!(!vm.flags.carry && vm.flags.zero);
    }

    #[test]
    fn asl_byte_carry_and_zero() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x80);
        set_b(&mut vm, 1, 1);
        op_asl(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0);
        assert!(vm.flags.carry && vm.flags.zero);
    }

    #[test]
    fn asl_negative_shift_keeps_carry() {
        let mut vm = vm();
        set_l(&mut vm, 0, 5);
        set_l(&mut vm, 1, (-1i32) as u32);
        vm.flags.carry = true;
        op_asl(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 5);
        assert!(vm.flags.carry);
    }

    #[test]
    fn lsr_byte_carry_from_last_shifted_bit() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x03);
        set_b(&mut vm, 1, 1);
        op_lsr(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 1);
        assert!(vm.flags.carry && !vm.flags.zero && !vm.flags.sign);
    }

    #[test]
    fn lsr_shift_equal_width_takes_top_bit_as_carry() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0x80);
        set_b(&mut vm, 1, 8);
        op_lsr(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0);
        assert!(vm.flags.carry && vm.flags.zero);
    }

    #[test]
    fn lsr_shift_past_width_no_carry() {
        let mut vm = vm();
        set_b(&mut vm, 0, 0xFF);
        set_b(&mut vm, 1, 9);
        op_lsr(&mut vm, oc(), &mut b(0), &mut b(1)).unwrap();
        assert_eq!(bv(&vm, 0), 0);
        assert!(!vm.flags.carry && vm.flags.zero);
    }

    #[test]
    fn addc_adds_carry_in() {
        let mut vm = vm();
        set_l(&mut vm, 0, 1);
        set_l(&mut vm, 1, 1);
        vm.flags.carry = true;
        op_addc(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 3);
        assert!(!vm.flags.carry);
    }

    #[test]
    fn addc_wraps_carry_in_operand() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0xFFFF_FFFF);
        set_l(&mut vm, 1, 0xFFFF_FFFF);
        vm.flags.carry = true;
        op_addc(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        // val1 + 1 wraps to 0, so the sum is 0xFFFFFFFF.
        assert_eq!(lv(&vm, 0), 0xFFFF_FFFF);
        assert!(!vm.flags.carry && vm.flags.sign && !vm.flags.zero);
    }

    #[test]
    fn subc_subtracts_borrow() {
        let mut vm = vm();
        set_l(&mut vm, 0, 5);
        set_l(&mut vm, 1, 3);
        vm.flags.carry = true;
        op_subc(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 1);
        assert!(!vm.flags.carry);
    }

    #[test]
    fn clrv_clears_overflow_only() {
        let mut vm = vm();
        vm.flags.overflow = true;
        vm.flags.carry = true;
        op_clrv(&mut vm, oc(), &mut Operand::none(), &mut Operand::none()).unwrap();
        assert!(!vm.flags.overflow && vm.flags.carry);
    }

    #[test]
    fn test_and_sets_zero_without_writing() {
        let mut vm = vm();
        set_l(&mut vm, 0, 0xF0);
        set_l(&mut vm, 1, 0x0F);
        vm.flags.carry = true;
        op_test(&mut vm, oc(), &mut l(0), &mut l(1)).unwrap();
        assert_eq!(lv(&vm, 0), 0xF0);
        assert!(vm.flags.zero && !vm.flags.overflow && vm.flags.carry);
    }
}
