//! Opcodes: move, clear, comp, subb, adds, mult, divs, and, or, xor, not, clrc, setc, asr, lsl, lsr, asl, addc, subc, clrv, test.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpMove` (move)
pub(crate) fn op_move(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("move"))
}

/// C#: `OpClear` (clear)
pub(crate) fn op_clear(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("clear"))
}

/// C#: `OpComp` (comp)
pub(crate) fn op_comp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("comp"))
}

/// C#: `OpSubb` (subb)
pub(crate) fn op_subb(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("subb"))
}

/// C#: `OpAdds` (adds)
pub(crate) fn op_adds(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("adds"))
}

/// C#: `OpMult` (mult)
pub(crate) fn op_mult(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("mult"))
}

/// C#: `OpDivs` (divs)
pub(crate) fn op_divs(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("divs"))
}

/// C#: `OpAnd` (and)
pub(crate) fn op_and(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("and"))
}

/// C#: `OpOr` (or)
pub(crate) fn op_or(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("or"))
}

/// C#: `OpXor` (xor)
pub(crate) fn op_xor(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xor"))
}

/// C#: `OpNot` (not)
pub(crate) fn op_not(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("not"))
}

/// C#: `OpClrc` (clrc)
pub(crate) fn op_clrc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("clrc"))
}

/// C#: `OpSetc` (setc)
pub(crate) fn op_setc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("setc"))
}

/// C#: `OpAsr` (asr)
pub(crate) fn op_asr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("asr"))
}

/// C#: `OpLsl` (lsl)
pub(crate) fn op_lsl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("lsl"))
}

/// C#: `OpLsr` (lsr)
pub(crate) fn op_lsr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("lsr"))
}

/// C#: `OpAsl` (asl)
pub(crate) fn op_asl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("asl"))
}

/// C#: `OpAddc` (addc)
pub(crate) fn op_addc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("addc"))
}

/// C#: `OpSubc` (subc)
pub(crate) fn op_subc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("subc"))
}

/// C#: `OpClrv` (clrv)
pub(crate) fn op_clrv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("clrv"))
}

/// C#: `OpTest` (test)
pub(crate) fn op_test(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("test"))
}
