//! Operands and their access (C#: `Operand`). The methods live on `Vm`
//! because reading or writing an operand touches the register file and
//! may raise `EDIABAS_BIP_0001`.

use crate::regs::Reg;
use crate::vm::Vm;
use crate::{ENCODING, ErrorCode, VmResult, internal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AddrMode {
    None = 0,
    RegS = 1,
    RegAb = 2,
    RegI = 3,
    RegL = 4,
    Imm8 = 5,
    Imm16 = 6,
    Imm32 = 7,
    ImmStr = 8,
    IdxImm = 9,
    IdxReg = 10,
    IdxRegImm = 11,
    IdxImmLenImm = 12,
    IdxImmLenReg = 13,
    IdxRegLenImm = 14,
    IdxRegLenReg = 15,
}

impl AddrMode {
    pub fn from_nibble(n: u8) -> AddrMode {
        use AddrMode::*;
        [
            None, RegS, RegAb, RegI, RegL, Imm8, Imm16, Imm32, ImmStr, IdxImm, IdxReg, IdxRegImm, IdxImmLenImm,
            IdxImmLenReg, IdxRegLenImm, IdxRegLenReg,
        ][(n & 0x0F) as usize]
    }

    /// Modes whose data type is a byte array (C#: `GetDataType() == typeof(byte[])`).
    pub fn is_array(self) -> bool {
        use AddrMode::*;
        matches!(
            self,
            RegS | ImmStr | IdxImm | IdxReg | IdxRegImm | IdxImmLenImm | IdxImmLenReg | IdxRegLenImm | IdxRegLenReg
        )
    }
}

/// C#: `OpData1/2/3` (typed `Object`).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum OpData {
    #[default]
    None,
    Reg(Reg),
    Val(u32),
    Bytes(Vec<u8>),
}

/// Raw operand value (C#: the `Object` returned by `GetRawData`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// C#: `EdValueType` (UInt32).
    Int(u32),
    /// C#: `EdFloatType` (double).
    Float(f64),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operand {
    pub mode: AddrMode,
    pub d1: OpData,
    pub d2: OpData,
    pub d3: OpData,
}

impl Default for Operand {
    fn default() -> Self {
        Self { mode: AddrMode::None, d1: OpData::None, d2: OpData::None, d3: OpData::None }
    }
}

/// Mirrors C# `GetDataType()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Value,
    Array,
    Float,
}

impl Operand {
    pub fn new(mode: AddrMode, d1: OpData, d2: OpData, d3: OpData) -> Self {
        Self { mode, d1, d2, d3 }
    }

    /// Register operand with the matching address mode.
    pub fn r(reg: Reg) -> Self {
        let mode = match reg {
            Reg::B(_) => AddrMode::RegAb,
            Reg::I(_) => AddrMode::RegI,
            Reg::L(_) => AddrMode::RegL,
            // C# encodes F registers with the RegS..RegL modes too; RegL
            // makes GetDataType() report the register's float type.
            Reg::F(_) => AddrMode::RegL,
            Reg::S(_) => AddrMode::RegS,
        };
        Self::new(mode, OpData::Reg(reg), OpData::None, OpData::None)
    }

    pub fn imm8(v: u8) -> Self {
        Self::new(AddrMode::Imm8, OpData::Val(v as u32), OpData::None, OpData::None)
    }

    pub fn imm16(v: u16) -> Self {
        Self::new(AddrMode::Imm16, OpData::Val(v as u32), OpData::None, OpData::None)
    }

    pub fn imm32(v: u32) -> Self {
        Self::new(AddrMode::Imm32, OpData::Val(v), OpData::None, OpData::None)
    }

    pub fn imm_str(b: &[u8]) -> Self {
        Self::new(AddrMode::ImmStr, OpData::Bytes(b.to_vec()), OpData::None, OpData::None)
    }

    /// `S[idx]`: one element of a string register (C# mode IdxImm).
    pub fn idx(reg: Reg, idx: u32) -> Self {
        Self::new(AddrMode::IdxImm, OpData::Reg(reg), OpData::Val(idx), OpData::None)
    }

    pub fn none() -> Self {
        Self::default()
    }

    /// The register in `OpData1`, if any (C#: `arg0.OpData1.GetType() == typeof(Register)`).
    pub fn reg(&self) -> Option<Reg> {
        match self.d1 {
            OpData::Reg(r) => Some(r),
            _ => None,
        }
    }

    /// C#: `Operand.GetDataType()`. Register operands of float registers
    /// report `Float`, as C# `Register.GetDataType` does.
    pub fn data_type(&self) -> DataType {
        if self.mode.is_array() {
            return DataType::Array;
        }
        match self.d1 {
            OpData::Reg(Reg::F(_)) => DataType::Float,
            _ => DataType::Value,
        }
    }

    /// C#: `arg0.OpData1` as `EdValueType`, e.g. a jump target.
    pub fn imm(&self) -> VmResult<u32> {
        match self.d1 {
            OpData::Val(v) => Ok(v),
            _ => internal("Operand: OpData1 is not a value"),
        }
    }
}

fn mask(len: u32) -> VmResult<u32> {
    match len {
        1 => Ok(0xFF),
        2 => Ok(0xFFFF),
        4 => Ok(0xFFFF_FFFF),
        _ => internal("Operand.GetValueMask: invalid length"),
    }
}

impl Vm {
    fn opdata_value(&self, d: &OpData) -> VmResult<u32> {
        match d {
            OpData::Val(v) => Ok(*v),
            OpData::Reg(r) => self.regs.value(*r),
            _ => internal("Operand: invalid index data type"),
        }
    }

    /// C#: `Operand.GetDataLen(write)`.
    pub fn op_data_len(&mut self, op: &Operand, write: bool) -> VmResult<u32> {
        use AddrMode::*;
        Ok(match op.mode {
            RegAb | Imm8 => 1,
            RegI | Imm16 => 2,
            RegL | Imm32 => 4,
            IdxImm | IdxReg | IdxRegImm if write => 1,
            RegS | ImmStr | IdxImm | IdxReg | IdxRegImm | IdxImmLenImm | IdxImmLenReg | IdxRegLenImm | IdxRegLenReg => {
                self.op_array(op)?.len() as u32
            }
            None => 0,
        })
    }

    /// C#: `Operand.GetRawData()`.
    pub fn op_raw(&mut self, op: &Operand) -> VmResult<Value> {
        use AddrMode::*;
        match op.mode {
            RegS | RegAb | RegI | RegL => {
                let r = op.reg().ok_or_else(|| crate::VmError::Internal("Operand.GetRawData RegX: invalid data type".into()))?;
                Ok(match r {
                    Reg::S(_) => Value::Bytes(self.regs.string(r)?.get(false)),
                    Reg::F(_) => Value::Float(self.regs.float(r)?),
                    _ => Value::Int(self.regs.value(r)?),
                })
            }
            Imm8 | Imm16 | Imm32 => Ok(Value::Int(op.imm()?)),
            ImmStr => match &op.d1 {
                OpData::Bytes(b) => Ok(Value::Bytes(b.clone())),
                _ => internal("Operand.GetRawData ImmStr: invalid data type"),
            },
            IdxImm | IdxReg | IdxRegImm => {
                let Some(r) = op.reg() else { return internal("Operand.GetRawData IdxX: invalid data type") };
                let data = self.regs.string(r)?.get(true);
                let mut index = self.opdata_value(&op.d2)?;
                if op.mode == IdxRegImm {
                    index = index.wrapping_add(self.opdata_value(&op.d3)?);
                }
                let required = index as u64 + 1;
                if required > self.array_max_size() as u64 {
                    self.set_error(ErrorCode::EDIABAS_BIP_0001)?;
                    return Ok(Value::Bytes(Vec::new()));
                }
                if (data.len() as u64) < required {
                    return Ok(Value::Bytes(Vec::new()));
                }
                Ok(Value::Bytes(data[index as usize..].to_vec()))
            }
            IdxImmLenImm | IdxImmLenReg | IdxRegLenImm | IdxRegLenReg => {
                let Some(r) = op.reg() else { return internal("Operand.GetRawData IdxXLenX: invalid data type") };
                let data = self.regs.string(r)?.get(true);
                let index = self.opdata_value(&op.d2)?;
                let mut len = self.opdata_value(&op.d3)?;
                let required = index as u64 + len as u64;
                if required > self.array_max_size() as u64 {
                    self.set_error(ErrorCode::EDIABAS_BIP_0001)?;
                    return Ok(Value::Bytes(Vec::new()));
                }
                if (data.len() as u64) < required {
                    if data.len() as u64 <= index as u64 {
                        return Ok(Value::Bytes(Vec::new()));
                    }
                    len = data.len() as u32 - index;
                }
                Ok(Value::Bytes(data[index as usize..(index + len) as usize].to_vec()))
            }
            None => internal("Operand.GetRawData: invalid address mode"),
        }
    }

    /// C#: `Operand.GetValueData(dataLen)`. `data_len` 0 means "use the
    /// operand's own length"; it is required for array operands, which are
    /// read little endian.
    pub fn op_value(&mut self, op: &Operand, data_len: u32) -> VmResult<u32> {
        match self.op_raw(op)? {
            Value::Int(v) => {
                let len = self.op_data_len(op, false)?;
                Ok(v & mask(len)?)
            }
            Value::Bytes(mut b) => {
                if data_len == 0 {
                    return internal("Operand.GetValueData: invalid data length");
                }
                b.resize(b.len().max(data_len as usize), 0);
                Ok(b[..data_len as usize].iter().rev().fold(0u32, |acc, &x| acc << 8 | x as u32))
            }
            Value::Float(_) => internal("Operand.GetValueData: invalid data type"),
        }
    }

    pub fn op_float(&mut self, op: &Operand) -> VmResult<f64> {
        match self.op_raw(op)? {
            Value::Float(f) => Ok(f),
            _ => internal("Operand.GetFloatData: invalid data type"),
        }
    }

    /// C#: `Operand.GetArrayData()`.
    pub fn op_array(&mut self, op: &Operand) -> VmResult<Vec<u8>> {
        match self.op_raw(op)? {
            Value::Bytes(b) => Ok(b),
            _ => internal("Operand.GetArrayData: invalid data type"),
        }
    }

    /// C#: `Operand.GetStringData()`: bytes up to the first NUL, Windows-1252.
    pub fn op_string(&mut self, op: &Operand) -> VmResult<String> {
        let data = self.op_array(op)?;
        let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        Ok(ENCODING.decode_without_bom_handling(&data[..end]).0.into_owned())
    }

    /// C#: `Operand.SetRawData(data, dataLen)`. `data_len` is the byte
    /// count when writing an `Int` into an indexed array (C# default 1).
    pub fn op_set_raw(&mut self, op: &Operand, data: Value, data_len: u32) -> VmResult<()> {
        use AddrMode::*;
        match op.mode {
            RegS | RegAb | RegI | RegL => {
                let Some(r) = op.reg() else { return internal("Operand.SetRawData RegX: invalid data type") };
                self.reg_set_raw(r, data)
            }
            IdxImm | IdxReg | IdxRegImm => {
                let Some(r) = op.reg() else { return internal("Operand.SetRawData IdxX: invalid data type") };
                let mut array = self.regs.string(r)?.get(false);
                let mut index = self.opdata_value(&op.d2)?;
                if op.mode == IdxRegImm {
                    index = index.wrapping_add(self.opdata_value(&op.d3)?);
                }
                let source = match data {
                    Value::Int(v) => (0..data_len).map(|i| (v >> (i * 8)) as u8).collect(),
                    Value::Bytes(b) => b,
                    Value::Float(_) => return internal("Operand.SetRawData IdxX: invalid input data type"),
                };
                let required = index as u64 + source.len() as u64;
                if required > self.array_max_size() as u64 {
                    return self.set_error(ErrorCode::EDIABAS_BIP_0001);
                }
                if (array.len() as u64) < required {
                    array.resize(required as usize, 0);
                }
                array[index as usize..required as usize].copy_from_slice(&source);
                self.reg_set_raw(r, Value::Bytes(array))
            }
            _ => internal("Operand.SetRawData: invalid address mode"),
        }
    }

    /// C#: `Operand.SetArrayData` (RegS only).
    pub fn op_set_array(&mut self, op: &Operand, data: Vec<u8>) -> VmResult<()> {
        match (op.mode, op.reg()) {
            (AddrMode::RegS, Some(r)) => self.reg_set_raw(r, Value::Bytes(data)),
            _ => internal("Operand.SetArrayData: invalid address mode"),
        }
    }

    /// C#: `Operand.SetStringData`: Windows-1252 plus a trailing NUL.
    pub fn op_set_string(&mut self, op: &Operand, s: &str) -> VmResult<()> {
        let mut data = ENCODING.encode(s).0.into_owned();
        if data.last().is_some_and(|&b| b != 0) {
            data.push(0);
        }
        self.op_set_array(op, data)
    }

    /// C#: `Register.SetRawData`.
    pub fn reg_set_raw(&mut self, r: Reg, data: Value) -> VmResult<()> {
        match (r, data) {
            (Reg::S(_), Value::Bytes(b)) => self.reg_set_array(r, &b, false),
            (Reg::F(_), Value::Float(f)) => self.regs.set_float(r, f),
            (Reg::B(_) | Reg::I(_) | Reg::L(_), Value::Int(v)) => self.regs.set_value(r, v),
            _ => internal("Register.SetRawData: invalid type"),
        }
    }

    /// C#: `Register.SetArrayData(value, keepLength)`.
    pub fn reg_set_array(&mut self, r: Reg, data: &[u8], keep_length: bool) -> VmResult<()> {
        if !self.regs.string_mut(r)?.set(data, keep_length) {
            return self.set_error(ErrorCode::EDIABAS_BIP_0001);
        }
        Ok(())
    }
}
