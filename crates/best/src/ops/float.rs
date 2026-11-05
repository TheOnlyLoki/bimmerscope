//! Opcodes: a2flt, fadd, fsub, fmul, fdiv, fix2flt, flt2a, setflt, flt2fix, flt2y4, flt2y8, y42flt, y82flt, fcomp.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpA2Flt` (a2flt)
pub(crate) fn op_a2flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("a2flt"))
}

/// C#: `OpFadd` (fadd)
pub(crate) fn op_fadd(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fadd"))
}

/// C#: `OpFsub` (fsub)
pub(crate) fn op_fsub(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fsub"))
}

/// C#: `OpFmul` (fmul)
pub(crate) fn op_fmul(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fmul"))
}

/// C#: `OpFdiv` (fdiv)
pub(crate) fn op_fdiv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fdiv"))
}

/// C#: `OpFix2Flt` (fix2flt)
pub(crate) fn op_fix2flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fix2flt"))
}

/// C#: `OpFlt2A` (flt2a)
pub(crate) fn op_flt2a(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("flt2a"))
}

/// C#: `OpSetflt` (setflt)
pub(crate) fn op_setflt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("setflt"))
}

/// C#: `OpFlt2Fix` (flt2fix)
pub(crate) fn op_flt2fix(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("flt2fix"))
}

/// C#: `OpFlt2Y4` (flt2y4)
pub(crate) fn op_flt2y4(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("flt2y4"))
}

/// C#: `OpFlt2Y8` (flt2y8)
pub(crate) fn op_flt2y8(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("flt2y8"))
}

/// C#: `OpY42Flt` (y42flt)
pub(crate) fn op_y42flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("y42flt"))
}

/// C#: `OpY82Flt` (y82flt)
pub(crate) fn op_y82flt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("y82flt"))
}

/// C#: `OpFcomp` (fcomp)
pub(crate) fn op_fcomp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fcomp"))
}
