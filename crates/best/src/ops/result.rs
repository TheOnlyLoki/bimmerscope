//! Opcodes: ergb, ergw, ergd, ergi, ergr, ergs, ergy, enewset, etag, eerr, ergc, ergl, ergsysi, generr.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpErgb` (ergb)
pub(crate) fn op_ergb(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergb"))
}

/// C#: `OpErgw` (ergw)
pub(crate) fn op_ergw(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergw"))
}

/// C#: `OpErgd` (ergd)
pub(crate) fn op_ergd(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergd"))
}

/// C#: `OpErgi` (ergi)
pub(crate) fn op_ergi(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergi"))
}

/// C#: `OpErgr` (ergr)
pub(crate) fn op_ergr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergr"))
}

/// C#: `OpErgs` (ergs)
pub(crate) fn op_ergs(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergs"))
}

/// C#: `OpErgy` (ergy)
pub(crate) fn op_ergy(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergy"))
}

/// C#: `OpEnewset` (enewset)
pub(crate) fn op_enewset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("enewset"))
}

/// C#: `OpEtag` (etag)
pub(crate) fn op_etag(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("etag"))
}

/// C#: `OpEerr` (eerr)
pub(crate) fn op_eerr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("eerr"))
}

/// C#: `OpErgc` (ergc)
pub(crate) fn op_ergc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergc"))
}

/// C#: `OpErgl` (ergl)
pub(crate) fn op_ergl(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergl"))
}

/// C#: `OpErgsysi` (ergsysi)
pub(crate) fn op_ergsysi(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ergsysi"))
}

/// C#: `OpGenerr` (generr)
pub(crate) fn op_generr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("generr"))
}
