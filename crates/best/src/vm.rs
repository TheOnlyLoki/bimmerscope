//! The interpreter state and fetch/decode/execute loop
//! (C#: `EdiabasNet.ExecuteJobPrivate`, `GetOpArg`).

use std::collections::HashMap;
use std::sync::Arc;

use sgbd::Sgbd;

use crate::flags::Flags;
use crate::iface::Interface;
use crate::operand::{AddrMode, OpData, Operand};
use crate::ops::{self, OPCODES};
use crate::regs::{Reg, Regs};
use crate::result::{ResultData, ResultSet};
use crate::{ErrorCode, VmError, VmResult, internal};

/// Default string register size when the SGBD header gives 0 (C#: 1024).
pub const DEFAULT_ARRAY_SIZE: u32 = 1024;
const XOR_KEY: u8 = 0xF7;

/// Job arguments (C#: `ArgString`/`ArgBinary` and the `Std` variants).
#[derive(Debug, Clone, Default)]
pub struct Args {
    /// Raw argument bytes. A string argument is stored as its Windows-1252 bytes.
    pub binary: Vec<u8>,
    pub binary_std: Vec<u8>,
}

pub struct Vm {
    /// C#: `_sgbdFs`: the loaded SGBD (tables, jobs).
    pub sgbd: Arc<Sgbd>,
    /// C#: the `fs` passed to `ExecuteJobPrivate`: the file the running
    /// job's bytecode is read from. Same as `sgbd`, except for jobs
    /// inherited via `uses`, where it is the base file (C#: `_sgbdBaseFs`).
    pub code: Arc<Sgbd>,
    pub regs: Regs,
    pub flags: Flags,
    pub stack: Vec<u8>,
    /// C#: `_pcCounter`. Byte offset of the next instruction in the file.
    pub pc: u32,
    pub job_end: bool,

    /// C#: `_errorTrapBitNr`; -1 = no error.
    pub error_trap_bit_nr: i32,
    pub error_trap_mask: u32,
    pub error_code_last: ErrorCode,
    /// C#: `_jobStdExit`.
    pub job_std_exit: bool,
    /// C#: `_jobStd`: true while running a standard job (INITIALISIERUNG etc).
    pub job_std: bool,

    array_max_buf_size: u32,

    /// C#: `_resultDict`: the result set being built.
    pub results: ResultSet,
    /// C#: `_resultSetsTemp`: finished result sets (without the system set).
    pub result_sets: Vec<ResultSet>,
    /// C#: `_resultSysDict`.
    pub sys_results: ResultSet,
    /// C#: `_resultsRequestDict`. Empty = all results requested.
    pub requested_results: Vec<String>,
    /// C#: `_resultJobStatus`.
    pub result_job_status: String,

    pub args: Args,
    pub config: HashMap<String, String>,
    pub iface: Option<Box<dyn Interface>>,
    /// C#: `_requestInit`: run INITIALISIERUNG before the next job.
    pub request_init: bool,

    /// Per-group opcode state, owned by the `ops::*` modules.
    pub st_arith: ops::arith::State,
    pub st_flow: ops::flow::State,
    pub st_string: ops::string::State,
    pub st_convert: ops::convert::State,
    pub st_float: ops::float::State,
    pub st_result: ops::result::State,
    pub st_param: ops::param::State,
    pub st_table: ops::table::State,
    pub st_iface: ops::iface::State,
    pub st_misc: ops::misc::State,

    /// Executed instructions, for debugging. Only filled if `trace` is set.
    pub trace: Option<Vec<String>>,
}

impl Vm {
    pub fn new(sgbd: Arc<Sgbd>) -> Self {
        let mut vm = Self {
            code: sgbd.clone(),
            sgbd,
            regs: Regs::default(),
            flags: Flags::default(),
            stack: Vec::new(),
            pc: 0,
            job_end: false,
            error_trap_bit_nr: -1,
            error_trap_mask: 0,
            error_code_last: ErrorCode::EDIABAS_ERR_NONE,
            job_std_exit: false,
            job_std: false,
            array_max_buf_size: 0,
            results: ResultSet::default(),
            result_sets: Vec::new(),
            sys_results: ResultSet::default(),
            requested_results: Vec::new(),
            result_job_status: String::new(),
            args: Args::default(),
            config: HashMap::new(),
            iface: None,
            request_init: true,
            st_arith: Default::default(),
            st_flow: Default::default(),
            st_string: Default::default(),
            st_convert: Default::default(),
            st_float: Default::default(),
            st_result: Default::default(),
            st_param: Default::default(),
            st_table: Default::default(),
            st_iface: Default::default(),
            st_misc: Default::default(),
            trace: None,
        };
        vm.set_array_max_buf_size(vm.sgbd.array_size.max(1));
        vm
    }

    /// Loads a new SGBD. Bytecode is read from it too, until `code` is changed.
    pub fn set_sgbd(&mut self, sgbd: Arc<Sgbd>) {
        self.code = sgbd.clone();
        self.sgbd = sgbd;
    }

    /// A VM over an empty SGBD, for opcode unit tests.
    pub fn for_test() -> Self {
        Self::new(Arc::new(Sgbd::empty()))
    }

    /// C#: `ArrayMaxBufSize` setter.
    pub fn set_array_max_buf_size(&mut self, size: u32) {
        self.array_max_buf_size = size;
        for s in &mut self.regs.strings {
            s.new_array_length(size as usize);
        }
    }

    pub fn array_max_buf_size(&self) -> u32 {
        self.array_max_buf_size
    }

    /// C#: `ArrayMaxSize` = buffer size - 1 (room for the NUL).
    pub fn array_max_size(&self) -> u32 {
        self.array_max_buf_size.saturating_sub(1)
    }

    /// C#: `SetError`. Returns `Err(Raised)` unless the error is trapped
    /// by the job's error mask; use it as `vm.set_error(code)?`.
    pub fn set_error(&mut self, error: ErrorCode) -> VmResult<()> {
        if error == ErrorCode::EDIABAS_ERR_NONE {
            self.error_trap_bit_nr = -1;
            return Ok(());
        }
        self.error_trap_bit_nr = error.trap_bit() as i32;
        let active = (1u32 << self.error_trap_bit_nr) & !self.error_trap_mask;
        if active != 0 {
            return Err(self.raise_error(error));
        }
        Ok(())
    }

    /// C#: `RaiseError`. Returns the error to propagate.
    pub fn raise_error(&mut self, error: ErrorCode) -> VmError {
        let code = if self.job_std_exit {
            ErrorCode::EDIABAS_SYS_0018
        } else {
            error
        };
        self.error_code_last = code;
        VmError::Raised(code)
    }

    /// C#: `SetResultData`.
    pub fn set_result_data(&mut self, data: ResultData) {
        self.results.set(data);
    }

    /// C#: `SetSysResultData`.
    pub fn set_sys_result_data(&mut self, data: ResultData) {
        self.sys_results.set(data);
    }

    /// C#: `_resultsRequestDict` check: is this result wanted by the caller?
    pub fn is_result_requested(&self, name: &str) -> bool {
        self.requested_results.is_empty()
            || self
                .requested_results
                .iter()
                .any(|r| r.eq_ignore_ascii_case(name))
    }

    /// C#: `GetActiveArgBinary`.
    pub fn active_arg_binary(&self) -> &[u8] {
        if self.job_std {
            &self.args.binary_std
        } else {
            &self.args.binary
        }
    }

    /// C#: `GetActiveArgStrings`: the argument split at ';'.
    pub fn active_arg_strings(&self) -> Vec<String> {
        let bytes = self.active_arg_binary();
        if bytes.is_empty() {
            return Vec::new();
        }
        let text = crate::ENCODING.decode_without_bom_handling(bytes).0;
        text.split(';').map(str::to_string).collect()
    }

    pub fn config_property(&self, key: &str) -> Option<&str> {
        self.config
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn code(&self, off: u32, len: usize) -> VmResult<Vec<u8>> {
        let data = self.code.data();
        let off = off as usize;
        match data.get(off..off + len) {
            Some(b) => Ok(b.iter().map(|x| x ^ XOR_KEY).collect()),
            None => internal(format!("bytecode read past end at {off:#x}")),
        }
    }

    /// Decodes one operand at `self.pc` and advances it (C#: `GetOpArg`).
    fn fetch_operand(&mut self, mode: AddrMode) -> VmResult<Operand> {
        use AddrMode::*;
        let reg = |b: u8| Reg::from_code(b).map(OpData::Reg);
        let u16_at = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as u32;
        let size = match mode {
            None => 0,
            RegS | RegAb | RegI | RegL | Imm8 => 1,
            Imm16 | IdxReg => 2,
            IdxImm | IdxRegLenReg => 3,
            Imm32 | IdxRegImm | IdxImmLenReg | IdxRegLenImm => 4,
            IdxImmLenImm => 5,
            ImmStr => 2,
        };
        let b = self.code(self.pc, size)?;
        self.pc += size as u32;
        let (d1, d2, d3) = match mode {
            None => (OpData::None, OpData::None, OpData::None),
            RegS | RegAb | RegI | RegL => (reg(b[0])?, OpData::None, OpData::None),
            Imm8 => (OpData::Val(b[0] as u32), OpData::None, OpData::None),
            Imm16 => (OpData::Val(u16_at(&b, 0)), OpData::None, OpData::None),
            Imm32 => (
                OpData::Val(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                OpData::None,
                OpData::None,
            ),
            ImmStr => {
                let len = i16::from_le_bytes([b[0], b[1]]).max(0) as usize;
                let s = self.code(self.pc, len)?;
                self.pc += len as u32;
                (OpData::Bytes(s), OpData::None, OpData::None)
            }
            IdxImm => (reg(b[0])?, OpData::Val(u16_at(&b, 1)), OpData::None),
            IdxReg => (reg(b[0])?, reg(b[1])?, OpData::None),
            IdxRegImm => (reg(b[0])?, reg(b[1])?, OpData::Val(u16_at(&b, 2))),
            IdxImmLenImm => (
                reg(b[0])?,
                OpData::Val(u16_at(&b, 1)),
                OpData::Val(u16_at(&b, 3)),
            ),
            IdxImmLenReg => (reg(b[0])?, OpData::Val(u16_at(&b, 1)), reg(b[3])?),
            IdxRegLenImm => (reg(b[0])?, reg(b[1])?, OpData::Val(u16_at(&b, 2))),
            IdxRegLenReg => (reg(b[0])?, reg(b[1])?, reg(b[2])?),
        };
        Ok(Operand::new(mode, d1, d2, d3))
    }

    /// Runs bytecode from `address` until `eoj` (C#: the loop in
    /// `ExecuteJobPrivate`). Resets per-job state first.
    pub fn run(&mut self, address: u32) -> VmResult<()> {
        self.results.clear();
        self.sys_results.clear();
        self.result_sets.clear();
        self.stack.clear();
        self.flags.init();
        for s in &mut self.regs.strings {
            s.clear();
        }
        self.error_trap_bit_nr = -1;
        self.error_trap_mask = 0;
        self.error_code_last = ErrorCode::EDIABAS_ERR_NONE;
        self.result_job_status.clear();
        // C#: `ArrayMaxBufSize = jobInfo.ArraySize`, the array size of the
        // file that holds the job.
        self.set_array_max_buf_size(self.code.array_size.max(1));
        // C#: `CloseTableFs()`.
        self.st_table = Default::default();
        // C#: _infoProgressRange/_infoProgressPos = -1, _infoProgressText = "".
        self.st_misc.info_progress_range = -1;
        self.st_misc.info_progress_pos = -1;
        self.st_misc.info_progress_text.clear();
        self.pc = address;
        self.job_end = false;

        while !self.job_end {
            let pc_old = self.pc;
            let head = self.code(self.pc, 2)?;
            self.pc += 2;
            let Some(oc) = OPCODES.get(head[0] as usize) else {
                return internal(format!(
                    "opcode {:#04x} out of range at {pc_old:#x}",
                    head[0]
                ));
            };
            let mut a0 = self.fetch_operand(AddrMode::from_nibble(head[1] >> 4))?;
            let mut a1 = self.fetch_operand(AddrMode::from_nibble(head[1]))?;
            // Near jumps: Imm32 arg0 is relative to the next instruction.
            if oc.arg0_is_near_address && a0.mode == AddrMode::Imm32 {
                a0.d1 = OpData::Val(self.pc.wrapping_add(a0.imm()?));
            }
            if let Some(t) = &mut self.trace {
                t.push(format!("{pc_old:08X}: {} {a0:?} {a1:?}", oc.name));
            }
            let Some(func) = oc.func else {
                return Err(VmError::NotImplemented(oc.name));
            };
            func(self, oc, &mut a0, &mut a1)?;
        }
        if !self.results.is_empty() {
            let set = std::mem::take(&mut self.results);
            self.result_sets.push(set);
        }
        Ok(())
    }

    /// Runs a job by name from this SGBD (no init job, no system results,
    /// no `uses` jobs). See `crate::Ediabas` for the full job layer.
    pub fn execute_job(&mut self, name: &str) -> VmResult<()> {
        let job = self
            .sgbd
            .jobs
            .iter()
            .find(|j| j.name.eq_ignore_ascii_case(name));
        let address = job
            .ok_or_else(|| VmError::JobNotFound(name.to_string()))?
            .address;
        self.code = self.sgbd.clone();
        self.run(address)
    }
}
