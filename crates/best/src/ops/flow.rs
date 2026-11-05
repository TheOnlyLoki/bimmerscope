//! Opcodes: jump, jc, jae, jz, jnz, jv, jnv, jmi, jpl, nop, eoj, push, pop, jt, jnt, popf, pushf, atsp, jg, jge, jl, jle, ja, jbe, plink, plinkv, ppush, ppop, ppushflt, ppopflt, ppushy, ppopy, pjtsr.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpJump` (jump)
pub(crate) fn op_jump(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jump"))
}

/// C#: `OpJc` (jc)
pub(crate) fn op_jc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jc"))
}

/// C#: `OpJae` (jae)
pub(crate) fn op_jae(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jae"))
}

/// C#: `OpJz` (jz)
pub(crate) fn op_jz(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jz"))
}

/// C#: `OpJnz` (jnz)
pub(crate) fn op_jnz(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jnz"))
}

/// C#: `OpJv` (jv)
pub(crate) fn op_jv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jv"))
}

/// C#: `OpJnv` (jnv)
pub(crate) fn op_jnv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jnv"))
}

/// C#: `OpJmi` (jmi)
pub(crate) fn op_jmi(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jmi"))
}

/// C#: `OpJpl` (jpl)
pub(crate) fn op_jpl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jpl"))
}

/// C#: `OpNop` (nop)
pub(crate) fn op_nop(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("nop"))
}

/// C#: `OpEoj` (eoj)
pub(crate) fn op_eoj(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("eoj"))
}

/// C#: `OpPush` (push)
pub(crate) fn op_push(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("push"))
}

/// C#: `OpPop` (pop)
pub(crate) fn op_pop(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("pop"))
}

/// C#: `OpJt` (jt)
pub(crate) fn op_jt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jt"))
}

/// C#: `OpJnt` (jnt)
pub(crate) fn op_jnt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jnt"))
}

/// C#: `OpPopf` (popf)
pub(crate) fn op_popf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("popf"))
}

/// C#: `OpPushf` (pushf)
pub(crate) fn op_pushf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("pushf"))
}

/// C#: `OpAtsp` (atsp)
pub(crate) fn op_atsp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("atsp"))
}

/// C#: `OpJg` (jg)
pub(crate) fn op_jg(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jg"))
}

/// C#: `OpJge` (jge)
pub(crate) fn op_jge(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jge"))
}

/// C#: `OpJl` (jl)
pub(crate) fn op_jl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jl"))
}

/// C#: `OpJle` (jle)
pub(crate) fn op_jle(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jle"))
}

/// C#: `OpJa` (ja)
pub(crate) fn op_ja(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ja"))
}

/// C#: `OpJbe` (jbe)
pub(crate) fn op_jbe(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("jbe"))
}

/// C#: `OpPlink` (plink)
pub(crate) fn op_plink(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("plink"))
}

/// C#: `OpPlinkv` (plinkv)
pub(crate) fn op_plinkv(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("plinkv"))
}

/// C#: `OpPpush` (ppush)
pub(crate) fn op_ppush(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ppush"))
}

/// C#: `OpPpop` (ppop)
pub(crate) fn op_ppop(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ppop"))
}

/// C#: `OpPpushflt` (ppushflt)
pub(crate) fn op_ppushflt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ppushflt"))
}

/// C#: `OpPpopflt` (ppopflt)
pub(crate) fn op_ppopflt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ppopflt"))
}

/// C#: `OpPpushy` (ppushy)
pub(crate) fn op_ppushy(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ppushy"))
}

/// C#: `OpPpopy` (ppopy)
pub(crate) fn op_ppopy(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ppopy"))
}

/// C#: `OpPjtsr` (pjtsr)
pub(crate) fn op_pjtsr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("pjtsr"))
}
