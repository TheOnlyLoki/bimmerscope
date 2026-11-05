//! Opcodes: gettmr, settmr, sett, clrt, break, fclose, fopen, fread, freadln, fseek, fseekln, ftell, ftellln, wait, date, time, cfgig, cfgsg, cfgis, shmset, shmget, iupdate, irange, iincpos, ticks, waitex.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {}

/// C#: `OpGettmr` (gettmr)
pub(crate) fn op_gettmr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("gettmr"))
}

/// C#: `OpSettmr` (settmr)
pub(crate) fn op_settmr(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("settmr"))
}

/// C#: `OpSett` (sett)
pub(crate) fn op_sett(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("sett"))
}

/// C#: `OpClrt` (clrt)
pub(crate) fn op_clrt(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("clrt"))
}

/// C#: `OpBreak` (break)
pub(crate) fn op_break(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("break"))
}

/// C#: `OpFclose` (fclose)
pub(crate) fn op_fclose(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fclose"))
}

/// C#: `OpFopen` (fopen)
pub(crate) fn op_fopen(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fopen"))
}

/// C#: `OpFread` (fread)
pub(crate) fn op_fread(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fread"))
}

/// C#: `OpFreadln` (freadln)
pub(crate) fn op_freadln(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("freadln"))
}

/// C#: `OpFseek` (fseek)
pub(crate) fn op_fseek(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fseek"))
}

/// C#: `OpFseekln` (fseekln)
pub(crate) fn op_fseekln(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("fseekln"))
}

/// C#: `OpFtell` (ftell)
pub(crate) fn op_ftell(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ftell"))
}

/// C#: `OpFtellln` (ftellln)
pub(crate) fn op_ftellln(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ftellln"))
}

/// C#: `OpWait` (wait)
pub(crate) fn op_wait(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("wait"))
}

/// C#: `OpDate` (date)
pub(crate) fn op_date(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("date"))
}

/// C#: `OpTime` (time)
pub(crate) fn op_time(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("time"))
}

/// C#: `OpCfgig` (cfgig)
pub(crate) fn op_cfgig(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("cfgig"))
}

/// C#: `OpCfgsg` (cfgsg)
pub(crate) fn op_cfgsg(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("cfgsg"))
}

/// C#: `OpCfgis` (cfgis)
pub(crate) fn op_cfgis(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("cfgis"))
}

/// C#: `OpShmset` (shmset)
pub(crate) fn op_shmset(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("shmset"))
}

/// C#: `OpShmget` (shmget)
pub(crate) fn op_shmget(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("shmget"))
}

/// C#: `OpIupdate` (iupdate)
pub(crate) fn op_iupdate(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("iupdate"))
}

/// C#: `OpIrange` (irange)
pub(crate) fn op_irange(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("irange"))
}

/// C#: `OpIincpos` (iincpos)
pub(crate) fn op_iincpos(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("iincpos"))
}

/// C#: `OpTicks` (ticks)
pub(crate) fn op_ticks(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("ticks"))
}

/// C#: `OpWaitex` (waitex)
pub(crate) fn op_waitex(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("waitex"))
}
