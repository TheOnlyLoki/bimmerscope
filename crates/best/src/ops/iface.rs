//! Opcodes: xconnect, xhangup, xsetpar, xawlen, xsend, xsendf, xrequf, xstopf, xkeyb, xstate, xboot, xreset, xtype, xvers, xreps, xbatt, xgetport, xignit, xloopt, xprog, xraw, xsetport, xsireset.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpXconnect` (xconnect)
pub(crate) fn op_xconnect(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xconnect"))
}

/// C#: `OpXhangup` (xhangup)
pub(crate) fn op_xhangup(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xhangup"))
}

/// C#: `OpXsetpar` (xsetpar)
pub(crate) fn op_xsetpar(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xsetpar"))
}

/// C#: `OpXawlen` (xawlen)
pub(crate) fn op_xawlen(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xawlen"))
}

/// C#: `OpXsend` (xsend)
pub(crate) fn op_xsend(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xsend"))
}

/// C#: `OpXsendf` (xsendf)
pub(crate) fn op_xsendf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xsendf"))
}

/// C#: `OpXrequf` (xrequf)
pub(crate) fn op_xrequf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xrequf"))
}

/// C#: `OpXstopf` (xstopf)
pub(crate) fn op_xstopf(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xstopf"))
}

/// C#: `OpXkeyb` (xkeyb)
pub(crate) fn op_xkeyb(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xkeyb"))
}

/// C#: `OpXstate` (xstate)
pub(crate) fn op_xstate(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xstate"))
}

/// C#: `OpXboot` (xboot)
pub(crate) fn op_xboot(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xboot"))
}

/// C#: `OpXreset` (xreset)
pub(crate) fn op_xreset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xreset"))
}

/// C#: `OpXtype` (xtype)
pub(crate) fn op_xtype(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xtype"))
}

/// C#: `OpXvers` (xvers)
pub(crate) fn op_xvers(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xvers"))
}

/// C#: `OpXreps` (xreps)
pub(crate) fn op_xreps(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xreps"))
}

/// C#: `OpXbatt` (xbatt)
pub(crate) fn op_xbatt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xbatt"))
}

/// C#: `OpXgetport` (xgetport)
pub(crate) fn op_xgetport(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xgetport"))
}

/// C#: `OpXignit` (xignit)
pub(crate) fn op_xignit(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xignit"))
}

/// C#: `OpXloopt` (xloopt)
pub(crate) fn op_xloopt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xloopt"))
}

/// C#: `OpXprog` (xprog)
pub(crate) fn op_xprog(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xprog"))
}

/// C#: `OpXraw` (xraw)
pub(crate) fn op_xraw(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xraw"))
}

/// C#: `OpXsetport` (xsetport)
pub(crate) fn op_xsetport(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xsetport"))
}

/// C#: `OpXsireset` (xsireset)
pub(crate) fn op_xsireset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("xsireset"))
}
