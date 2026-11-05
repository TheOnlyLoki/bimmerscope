//! Opcodes: tabset, tabseek, tabget, tabline, tabseeku, tabsetex, tabcols, tabrows.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpTabset` (tabset)
pub(crate) fn op_tabset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabset"))
}

/// C#: `OpTabseek` (tabseek)
pub(crate) fn op_tabseek(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabseek"))
}

/// C#: `OpTabget` (tabget)
pub(crate) fn op_tabget(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabget"))
}

/// C#: `OpTabline` (tabline)
pub(crate) fn op_tabline(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabline"))
}

/// C#: `OpTabseeku` (tabseeku)
pub(crate) fn op_tabseeku(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabseeku"))
}

/// C#: `OpTabsetex` (tabsetex)
pub(crate) fn op_tabsetex(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabsetex"))
}

/// C#: `OpTabcols` (tabcols)
pub(crate) fn op_tabcols(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabcols"))
}

/// C#: `OpTabrows` (tabrows)
pub(crate) fn op_tabrows(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("tabrows"))
}
