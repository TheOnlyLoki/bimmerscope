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
    Err(VmError::NotImplemented("a2fix"))
}

/// C#: `OpFix2Hex` (fix2hex)
pub(crate) fn op_fix2hex(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fix2hex"))
}

/// C#: `OpFix2Dez` (fix2dez)
pub(crate) fn op_fix2dez(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fix2dez"))
}

/// C#: `OpA2Y` (a2y)
pub(crate) fn op_a2y(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("a2y"))
}

/// C#: `OpHex2Y` (hex2y)
pub(crate) fn op_hex2y(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("hex2y"))
}

/// C#: `OpY2Bcd` (y2bcd)
pub(crate) fn op_y2bcd(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("y2bcd"))
}

/// C#: `OpY2Hex` (y2hex)
pub(crate) fn op_y2hex(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("y2hex"))
}

/// C#: `OpUfix2Dez` (ufix2dez)
pub(crate) fn op_ufix2dez(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ufix2dez"))
}
