//! Register file. B/A (8 bit), I (16 bit) and L (32 bit) registers are
//! overlapping views of one 32-byte array, little endian. F registers hold
//! doubles, S registers hold byte strings.

use crate::{VmResult, internal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    /// 8 bit, index 0..32. Indexes 16.. are named A0..AF.
    B(u8),
    /// 16 bit, index 0..16.
    I(u8),
    /// 32 bit, index 0..8.
    L(u8),
    /// Float, index 0..8.
    F(u8),
    /// String, index 0..16.
    S(u8),
}

impl Reg {
    /// Decodes a register byte from the bytecode (C#: `GetRegister`).
    pub fn from_code(code: u8) -> VmResult<Reg> {
        Ok(match code {
            0x00..=0x0F => Reg::B(code),
            0x10..=0x17 => Reg::I(code - 0x10),
            0x18..=0x1B => Reg::L(code - 0x18),
            0x1C..=0x23 => Reg::S(code - 0x1C),
            0x24..=0x2B => Reg::F(code - 0x24),
            0x2C..=0x33 => Reg::S(code - 0x2C + 8),
            0x80..=0x8F => Reg::B(code - 0x80 + 16),
            0x90..=0x97 => Reg::I(code - 0x90 + 8),
            0x98..=0x9B => Reg::L(code - 0x98 + 4),
            _ => return internal(format!("GetRegister: opcode {code:#04x} out of range")),
        })
    }

    pub fn name(self) -> String {
        match self {
            Reg::B(i) if i > 15 => format!("A{:X}", i - 16),
            Reg::B(i) => format!("B{i:X}"),
            Reg::I(i) => format!("I{i:X}"),
            Reg::L(i) => format!("L{i:X}"),
            Reg::F(i) => format!("F{i:X}"),
            Reg::S(i) => format!("S{i:X}"),
        }
    }

    /// Byte length of an integer register; `None` for F and S.
    pub fn int_len(self) -> Option<u32> {
        match self {
            Reg::B(_) => Some(1),
            Reg::I(_) => Some(2),
            Reg::L(_) => Some(4),
            Reg::F(_) | Reg::S(_) => None,
        }
    }
}

/// One S register (C#: `StringData`). `data` is the full buffer of
/// `ArrayMaxBufSize` bytes; `len` is the used length.
#[derive(Debug, Clone, Default)]
pub struct StringData {
    pub data: Vec<u8>,
    pub len: usize,
}

impl StringData {
    pub fn new_array_length(&mut self, length: usize) {
        if length > self.data.len() {
            self.data = vec![0; length];
        }
    }

    /// C#: `GetData(complete)`. `complete` returns the whole buffer.
    pub fn get(&self, complete: bool) -> Vec<u8> {
        if complete { self.data.clone() } else { self.data[..self.len].to_vec() }
    }

    /// C#: `SetData`. Returns false if the value does not fit
    /// (the caller raises `EDIABAS_BIP_0001`).
    pub fn set(&mut self, value: &[u8], keep_length: bool) -> bool {
        if value.len() > self.data.len() {
            return false;
        }
        self.data[..value.len()].copy_from_slice(value);
        if !keep_length {
            self.len = value.len();
        }
        true
    }

    pub fn clear(&mut self) {
        self.data.fill(0);
        self.len = 0;
    }
}

#[derive(Debug, Clone)]
pub struct Regs {
    pub bytes: [u8; 32],
    pub floats: [f64; 16],
    pub strings: [StringData; 16],
}

impl Default for Regs {
    fn default() -> Self {
        Self { bytes: [0; 32], floats: [0.0; 16], strings: Default::default() }
    }
}

impl Regs {
    /// Integer register value (C#: `Register.GetValueData`).
    pub fn value(&self, r: Reg) -> VmResult<u32> {
        let b = &self.bytes;
        Ok(match r {
            Reg::B(i) => b[i as usize] as u32,
            Reg::I(i) => {
                let o = i as usize * 2;
                u16::from_le_bytes([b[o], b[o + 1]]) as u32
            }
            Reg::L(i) => {
                let o = i as usize * 4;
                u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
            }
            _ => return internal("Register.GetValueData: invalid data type"),
        })
    }

    /// C#: `Register.SetValueData`. Truncates to the register width.
    pub fn set_value(&mut self, r: Reg, v: u32) -> VmResult<()> {
        let b = &mut self.bytes;
        match r {
            Reg::B(i) => b[i as usize] = v as u8,
            Reg::I(i) => {
                let o = i as usize * 2;
                b[o..o + 2].copy_from_slice(&(v as u16).to_le_bytes());
            }
            Reg::L(i) => {
                let o = i as usize * 4;
                b[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
            _ => return internal("Register.SetValueData: invalid data type"),
        }
        Ok(())
    }

    pub fn float(&self, r: Reg) -> VmResult<f64> {
        match r {
            Reg::F(i) => Ok(self.floats[i as usize]),
            _ => internal("Register.GetFloatData: invalid data type"),
        }
    }

    pub fn set_float(&mut self, r: Reg, v: f64) -> VmResult<()> {
        match r {
            Reg::F(i) => {
                self.floats[i as usize] = v;
                Ok(())
            }
            _ => internal("Register.SetFloatData: invalid data type"),
        }
    }

    pub fn string(&self, r: Reg) -> VmResult<&StringData> {
        match r {
            Reg::S(i) => Ok(&self.strings[i as usize]),
            _ => internal("Register.GetArrayData: invalid data type"),
        }
    }

    pub fn string_mut(&mut self, r: Reg) -> VmResult<&mut StringData> {
        match r {
            Reg::S(i) => Ok(&mut self.strings[i as usize]),
            _ => internal("Register.GetArrayData: invalid data type"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_views_overlap() {
        let mut r = Regs::default();
        r.set_value(Reg::L(0), 0x1234_5678).unwrap();
        assert_eq!(r.value(Reg::B(0)).unwrap(), 0x78);
        assert_eq!(r.value(Reg::I(1)).unwrap(), 0x1234);
        r.set_value(Reg::B(16), 0xAB).unwrap();
        assert_eq!(r.value(Reg::L(4)).unwrap(), 0xAB);
    }

    #[test]
    fn register_codes() {
        assert_eq!(Reg::from_code(0x2C).unwrap(), Reg::S(8));
        assert_eq!(Reg::from_code(0x9B).unwrap(), Reg::L(7));
        assert_eq!(Reg::from_code(0x80).unwrap().name(), "A0");
        assert!(Reg::from_code(0x40).is_err());
    }
}
