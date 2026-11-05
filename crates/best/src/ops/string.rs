//! Opcodes: scmp, scat, scut, slen, spaste, serase, swap, setspc, srevrs, stoken, strcat, strcmp, strlen, ssize.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpScmp` (scmp)
pub(crate) fn op_scmp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("scmp"))
}

/// C#: `OpScat` (scat)
pub(crate) fn op_scat(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("scat"))
}

/// C#: `OpScut` (scut)
pub(crate) fn op_scut(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("scut"))
}

/// C#: `OpSlen` (slen)
pub(crate) fn op_slen(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("slen"))
}

/// C#: `OpSpaste` (spaste)
pub(crate) fn op_spaste(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("spaste"))
}

/// C#: `OpSerase` (serase)
pub(crate) fn op_serase(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("serase"))
}

/// C#: `OpSwap` (swap)
pub(crate) fn op_swap(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("swap"))
}

/// C#: `OpSetspc` (setspc)
pub(crate) fn op_setspc(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("setspc"))
}

/// C#: `OpSrevrs` (srevrs)
pub(crate) fn op_srevrs(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("srevrs"))
}

/// C#: `OpStoken` (stoken)
pub(crate) fn op_stoken(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("stoken"))
}

/// C#: `OpStrcat` (strcat)
pub(crate) fn op_strcat(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("strcat"))
}

/// C#: `OpStrcmp` (strcmp)
pub(crate) fn op_strcmp(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("strcmp"))
}

/// C#: `OpStrlen` (strlen)
pub(crate) fn op_strlen(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("strlen"))
}

/// C#: `OpSsize` (ssize)
pub(crate) fn op_ssize(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ssize"))
}
