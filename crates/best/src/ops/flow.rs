//! Opcodes: jump, jc, jae, jz, jnz, jv, jnv, jmi, jpl, nop, eoj, push, pop, jt, jnt, popf, pushf, atsp, jg, jge, jl, jle, ja, jbe, plink, plinkv, ppush, ppop, ppushflt, ppopflt, ppushy, ppopy, pjtsr.
//! Ported from `EdOperations.cs`.

use super::OpCode;
use crate::operand::{AddrMode, DataType, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmResult, internal};

/// Per-group VM state. None needed: the stack and flags live on `Vm`, and the
/// plugin opcodes (plink, pjtsr, ppush*, ...) only log in C#.
#[derive(Debug, Default)]
pub struct State {}

/// Pops one byte from the stack (C#: `_stackList.Pop()`).
fn pop_byte(vm: &mut Vm) -> VmResult<u8> {
    match vm.stack.pop() {
        Some(b) => Ok(b),
        None => internal("stack empty"),
    }
}

/// Element `k` of C# `_stackList.ToArray()`: index 0 is the top of the stack.
fn stack_from_top(vm: &Vm, k: u64) -> VmResult<u8> {
    let len = vm.stack.len() as u64;
    if k >= len {
        // C# indexes the array past its end and throws IndexOutOfRangeException.
        return internal("OpAtsp: stack index out of range");
    }
    Ok(vm.stack[(len - 1 - k) as usize])
}

/// C#: `ediabas._pcCounter = arg0.GetValueData()`, done only when `taken`.
fn jump_if(vm: &mut Vm, arg0: &Operand, taken: bool) -> VmResult<()> {
    if taken {
        vm.pc = vm.op_value(arg0, 0)?;
    }
    Ok(())
}

/// C#: `Flags.UpdateFlags` throws for lengths other than 1, 2 or 4.
/// The Rust `Flags` panics on those, so check first and return an error.
fn update_flags(vm: &mut Vm, value: u32, length: u32) -> VmResult<()> {
    if !matches!(length, 1 | 2 | 4) {
        return internal("Flags.UpdateFlags: Invalid length");
    }
    vm.flags.update_flags(value, length);
    Ok(())
}

/// C#: trap bit test of OpJt/OpJnt when arg1 is present.
fn trap_bit_match(vm: &mut Vm, arg1: &Operand) -> VmResult<bool> {
    let test_bit = vm.op_value(arg1, 1)?;
    if test_bit > 0 {
        let mut detected = vm.error_trap_bit_nr == test_bit as i32;
        if vm.error_trap_bit_nr == 0 && test_bit == 32 {
            detected = true;
        }
        Ok(detected)
    } else {
        Ok(vm.error_trap_bit_nr >= 0x4000_0000)
    }
}

/// C#: `OpJump` (jump)
pub(crate) fn op_jump(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    jump_if(vm, arg0, true)
}

/// C#: `OpJc` (jc)
pub(crate) fn op_jc(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.carry;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJae` (jae)
pub(crate) fn op_jae(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = !vm.flags.carry;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJz` (jz)
pub(crate) fn op_jz(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.zero;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJnz` (jnz)
pub(crate) fn op_jnz(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = !vm.flags.zero;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJv` (jv)
pub(crate) fn op_jv(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.overflow;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJnv` (jnv)
pub(crate) fn op_jnv(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = !vm.flags.overflow;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJmi` (jmi)
pub(crate) fn op_jmi(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.sign;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJpl` (jpl)
pub(crate) fn op_jpl(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = !vm.flags.sign;
    jump_if(vm, arg0, taken)
}

/// C#: `OpNop` (nop)
pub(crate) fn op_nop(_vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    Ok(())
}

/// C#: `OpEoj` (eoj)
pub(crate) fn op_eoj(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    if arg0.mode != AddrMode::None {
        vm.result_job_status = vm.op_string(arg0)?;
    }
    vm.job_end = true;
    Ok(())
}

/// C#: `OpPush` (push)
pub(crate) fn op_push(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let mut value = vm.op_value(arg0, 0)?;
    let length = vm.op_data_len(arg0, false)?;
    for _ in 0..length {
        vm.stack.push(value as u8);
        value >>= 8;
    }
    Ok(())
}

/// C#: `OpPop` (pop)
pub(crate) fn op_pop(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpPop: Invalid type");
    }
    if arg0.data_type() != DataType::Value {
        return internal("OpPop: Invalid data type");
    }

    let mut value: u32 = 0;
    let length = vm.op_data_len(arg0, false)?;
    if (vm.stack.len() as u64) < length as u64 {
        vm.set_error(ErrorCode::EDIABAS_BIP_0005)?;
    } else {
        for _ in 0..length {
            value = (value << 8) | pop_byte(vm)? as u32;
        }
    }

    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    vm.flags.overflow = false;
    update_flags(vm, value, length)
}

/// C#: `OpPopf` (popf)
pub(crate) fn op_popf(vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let mut value: u32 = 0;
    if vm.stack.len() < 4 {
        vm.set_error(ErrorCode::EDIABAS_BIP_0005)?;
    } else {
        for _ in 0..4 {
            value = (value << 8) | pop_byte(vm)? as u32;
        }
    }
    vm.flags.from_value(value);
    Ok(())
}

/// C#: `OpPushf` (pushf)
pub(crate) fn op_pushf(vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let mut value = vm.flags.to_value();
    for _ in 0..4 {
        vm.stack.push(value as u8);
        value >>= 8;
    }
    Ok(())
}

/// C#: `OpAtsp` (atsp)
pub(crate) fn op_atsp(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpAtsp: Invalid type");
    }
    if arg0.data_type() != DataType::Value {
        return internal("OpAtsp: Invalid data type");
    }

    let mut value: u32 = 0;
    let length = vm.op_data_len(arg0, false)?;
    let pos = vm.op_value(arg1, 0)?;
    if (vm.stack.len() as u64) < length as u64 {
        vm.set_error(ErrorCode::EDIABAS_BIP_0005)?;
    } else {
        // C#: `pos - length` is uint arithmetic and wraps before widening to long,
        // so an out-of-range pos does not hit the "Invalid stack index" check.
        // The read then fails on the array bounds instead.
        let mut index = pos.wrapping_sub(length) as u64;
        for _ in 0..length {
            value = (value << 8) | stack_from_top(vm, index)? as u32;
            index += 1;
        }
    }

    vm.op_set_raw(arg0, Value::Int(value), 1)?;
    update_flags(vm, value, length)
}

/// C#: `OpJg` (jg)
pub(crate) fn op_jg(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.sign == vm.flags.overflow && !vm.flags.zero;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJge` (jge)
pub(crate) fn op_jge(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.zero || vm.flags.sign == vm.flags.overflow;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJl` (jl)
pub(crate) fn op_jl(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = !vm.flags.zero && vm.flags.sign != vm.flags.overflow;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJle` (jle)
pub(crate) fn op_jle(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.sign != vm.flags.overflow || vm.flags.zero;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJa` (ja)
pub(crate) fn op_ja(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = !vm.flags.carry && !vm.flags.zero;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJbe` (jbe)
pub(crate) fn op_jbe(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let taken = vm.flags.carry || vm.flags.zero;
    jump_if(vm, arg0, taken)
}

/// C#: `OpJt` (jt): jump if the error trap bit is set.
pub(crate) fn op_jt(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    // C# does not reset `_errorTrapBitNr` here.
    let detected = if arg1.mode != AddrMode::None {
        trap_bit_match(vm, arg1)?
    } else {
        vm.error_trap_bit_nr >= 0
    };
    jump_if(vm, arg0, detected)
}

/// C#: `OpJnt` (jnt): jump if the error trap bit is not set.
pub(crate) fn op_jnt(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let detected = if arg1.mode != AddrMode::None {
        trap_bit_match(vm, arg1)?
    } else {
        // C#: same as OpJt, but the no-argument case tests the "no error" marker.
        vm.error_trap_bit_nr >= 0x4000_0000
    };
    jump_if(vm, arg0, !detected)
}

/// C#: `OpPlink` (plink). C# only logs, but the log argument is evaluated
/// (side effects, possible errors), so the operand is read here too.
pub(crate) fn op_plink(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    vm.op_string(arg0)?;
    Ok(())
}

/// C#: `OpPlinkv` (plinkv). Same as plink; arg1 is read for the version log argument.
pub(crate) fn op_plinkv(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    vm.op_string(arg0)?;
    vm.op_value(arg1, 0)?;
    Ok(())
}

/// C#: `OpPpush` (ppush). Plugin stub: ignored.
pub(crate) fn op_ppush(_vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    Ok(())
}

/// C#: `OpPpop` (ppop). Plugin stub: pops nothing, writes 0 to arg0.
pub(crate) fn op_ppop(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    vm.op_set_raw(arg0, Value::Int(0), 1)?;
    vm.flags.overflow = false;
    let length = vm.op_data_len(arg0, false)?;
    update_flags(vm, 0, length)
}

/// C#: `OpPpushflt` (ppushflt). Plugin stub: ignored.
pub(crate) fn op_ppushflt(_vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    Ok(())
}

/// C#: `OpPpopflt` (ppopflt). Plugin stub: writes 0.0 to arg0.
pub(crate) fn op_ppopflt(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    vm.op_set_raw(arg0, Value::Float(0.0), 1)?;
    vm.flags.overflow = false;
    Ok(())
}

/// C#: `OpPpushy` (ppushy). Plugin stub: ignored.
pub(crate) fn op_ppushy(_vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    Ok(())
}

/// C#: `OpPpopy` (ppopy). Plugin stub: writes an empty string to arg0.
pub(crate) fn op_ppopy(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    vm.op_set_array(arg0, Vec::new())
}

/// C#: `OpPjtsr` (pjtsr). Plugin jump stub: ignored.
pub(crate) fn op_pjtsr(_vm: &mut Vm, _oc: &OpCode, _arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VmError;
    use crate::flags::Flags;
    use crate::ops::{OPCODES, OpFn};
    use crate::regs::Reg;

    fn run(f: OpFn, vm: &mut Vm, a0: &mut Operand, a1: &mut Operand) -> VmResult<()> {
        f(vm, &OPCODES[0], a0, a1)
    }

    #[test]
    fn conditional_jumps_follow_flags() {
        let cases: [(OpFn, Flags, bool); 8] = [
            (op_jz, Flags { zero: true, ..Flags::default() }, true),
            (op_jz, Flags::default(), false),
            (op_jnz, Flags::default(), true),
            (op_jnz, Flags { zero: true, ..Flags::default() }, false),
            (op_jc, Flags { carry: true, ..Flags::default() }, true),
            (op_jc, Flags::default(), false),
            (op_jg, Flags::default(), true),
            (op_jg, Flags { sign: true, ..Flags::default() }, false),
        ];
        for (f, flags, taken) in cases {
            let mut vm = Vm::for_test();
            vm.pc = 0x10;
            vm.flags = flags;
            let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::none());
            run(f, &mut vm, &mut a0, &mut a1).unwrap();
            assert_eq!(vm.pc, if taken { 0x200 } else { 0x10 }, "flags {flags:?}");
        }
    }

    #[test]
    fn jump_is_unconditional_and_accepts_register_target() {
        let mut vm = Vm::for_test();
        vm.regs.set_value(Reg::L(0), 0x345).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::none());
        run(op_jump, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x345);
    }

    #[test]
    fn jt_jnt_check_trap_bit() {
        // Trap bit 5 is set: jt taken for bit 5, jnt taken for bit 6.
        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 5;
        vm.pc = 0x10;
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::imm8(5));
        run(op_jt, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x200);

        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 5;
        vm.pc = 0x10;
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::imm8(6));
        run(op_jt, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x10);

        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 5;
        vm.pc = 0x10;
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::imm8(6));
        run(op_jnt, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x200);

        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 5;
        vm.pc = 0x10;
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::imm8(5));
        run(op_jnt, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x10);

        // No argument: jt taken when any trap is set, jnt when none is set.
        let mut vm = Vm::for_test();
        vm.error_trap_bit_nr = 0;
        vm.pc = 0x10;
        let (mut a0, mut a1) = (Operand::imm32(0x200), Operand::none());
        run(op_jt, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x200);
        vm.pc = 0x10;
        run(op_jnt, &mut vm, &mut a0, &mut a1).unwrap();
        assert_eq!(vm.pc, 0x200);
    }

    #[test]
    fn push_pop_round_trip_b_i_l() {
        let mut vm = Vm::for_test();
        vm.regs.set_value(Reg::L(0), 0x1122_3344).unwrap();
        run(op_push, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.stack, vec![0x44, 0x33, 0x22, 0x11]);
        run(op_pop, &mut vm, &mut Operand::r(Reg::L(1)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::L(1)).unwrap(), 0x1122_3344);
        assert!(vm.stack.is_empty());
        assert!(!vm.flags.zero && !vm.flags.sign);

        vm.regs.set_value(Reg::I(0), 0x8001).unwrap();
        run(op_push, &mut vm, &mut Operand::r(Reg::I(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.stack, vec![0x01, 0x80]);
        run(op_pop, &mut vm, &mut Operand::r(Reg::I(1)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::I(1)).unwrap(), 0x8001);
        assert!(vm.flags.sign);

        vm.regs.set_value(Reg::B(0), 0x80).unwrap();
        run(op_push, &mut vm, &mut Operand::r(Reg::B(0)), &mut Operand::none()).unwrap();
        run(op_pop, &mut vm, &mut Operand::r(Reg::B(1)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::B(1)).unwrap(), 0x80);
        assert!(vm.flags.sign && !vm.flags.zero);
    }

    #[test]
    fn pop_on_empty_stack_raises_bip_0005() {
        let mut vm = Vm::for_test();
        let res = run(op_pop, &mut vm, &mut Operand::r(Reg::B(0)), &mut Operand::none());
        assert!(matches!(res, Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0005))));
    }

    #[test]
    fn pushf_popf_round_trip() {
        let mut vm = Vm::for_test();
        vm.flags = Flags { carry: true, zero: false, sign: true, overflow: true };
        run(op_pushf, &mut vm, &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(vm.stack, vec![0x0D, 0, 0, 0]);
        vm.flags = Flags::default();
        run(op_popf, &mut vm, &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(vm.flags, Flags { carry: true, zero: false, sign: true, overflow: true });
        assert!(vm.stack.is_empty());
    }

    #[test]
    fn atsp_reads_from_top_of_stack() {
        let mut vm = Vm::for_test();
        vm.regs.set_value(Reg::L(0), 0x4433_2211).unwrap();
        run(op_push, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        // Stack bottom to top: 11 22 33 44. pos 4 with length 1 reads 0x11.
        run(op_atsp, &mut vm, &mut Operand::r(Reg::B(0)), &mut Operand::imm8(4)).unwrap();
        assert_eq!(vm.regs.value(Reg::B(0)).unwrap(), 0x11);
        assert!(!vm.flags.zero && !vm.flags.sign);
        // Length 2 at pos 3: bytes 0x33 then 0x22 give 0x3322.
        run(op_atsp, &mut vm, &mut Operand::r(Reg::I(0)), &mut Operand::imm8(3)).unwrap();
        assert_eq!(vm.regs.value(Reg::I(0)).unwrap(), 0x3322);
        // pos 0 wraps the index (C# uint arithmetic) and fails on the array bounds.
        let res = run(op_atsp, &mut vm, &mut Operand::r(Reg::B(0)), &mut Operand::imm8(0));
        assert!(res.is_err());
    }

    #[test]
    fn eoj_sets_job_end_and_status() {
        let mut vm = Vm::for_test();
        run(op_eoj, &mut vm, &mut Operand::imm_str(b"OK"), &mut Operand::none()).unwrap();
        assert!(vm.job_end);
        assert_eq!(vm.result_job_status, "OK");

        let mut vm = Vm::for_test();
        run(op_eoj, &mut vm, &mut Operand::none(), &mut Operand::none()).unwrap();
        assert!(vm.job_end);
        assert_eq!(vm.result_job_status, "");
    }

    #[test]
    fn plugin_stubs_clear_operands() {
        let mut vm = Vm::for_test();
        vm.regs.set_value(Reg::B(0), 5).unwrap();
        vm.flags.overflow = true;
        run(op_ppop, &mut vm, &mut Operand::r(Reg::B(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::B(0)).unwrap(), 0);
        assert!(vm.flags.zero && !vm.flags.overflow);

        vm.flags.overflow = true;
        run(op_ppopflt, &mut vm, &mut Operand::r(Reg::F(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.float(Reg::F(0)).unwrap(), 0.0);
        assert!(!vm.flags.overflow);

        vm.regs.set_value(Reg::L(0), 7).unwrap();
        vm.stack.push(9);
        run(op_ppush, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        run(op_ppushflt, &mut vm, &mut Operand::r(Reg::F(0)), &mut Operand::none()).unwrap();
        run(op_ppushy, &mut vm, &mut Operand::imm_str(b"x"), &mut Operand::none()).unwrap();
        run(op_pjtsr, &mut vm, &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 7);
        assert_eq!(vm.stack, vec![9]);
        run(op_plink, &mut vm, &mut Operand::imm_str(b"plugin"), &mut Operand::none()).unwrap();
        run(op_plinkv, &mut vm, &mut Operand::imm_str(b"plugin"), &mut Operand::imm16(0x0100)).unwrap();
    }

    #[test]
    fn nop_has_no_effect() {
        let mut vm = Vm::for_test();
        vm.pc = 0x42;
        run(op_nop, &mut vm, &mut Operand::none(), &mut Operand::none()).unwrap();
        assert_eq!(vm.pc, 0x42);
    }
}
