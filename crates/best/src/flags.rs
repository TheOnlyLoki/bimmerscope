//! CPU flags (C#: `Flags`).

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub carry: bool,
    pub zero: bool,
    pub sign: bool,
    pub overflow: bool,
}

fn sign_mask(length: u32) -> u64 {
    match length {
        1 => 0x80,
        2 => 0x8000,
        4 => 0x8000_0000,
        _ => panic!("Flags: invalid length {length}"),
    }
}

impl Flags {
    pub fn init(&mut self) {
        *self = Self::default();
    }

    /// Sets zero and sign from `value` truncated to `length` bytes.
    pub fn update_flags(&mut self, value: u32, length: u32) {
        let value_mask = match length {
            1 => 0xFF,
            2 => 0xFFFF,
            4 => 0xFFFF_FFFF,
            _ => panic!("Flags.UpdateFlags: invalid length {length}"),
        };
        self.zero = value & value_mask == 0;
        self.sign = value as u64 & sign_mask(length) != 0;
    }

    pub fn set_overflow(&mut self, value1: u32, value2: u32, result: u32, length: u32) {
        let m = sign_mask(length);
        let (s1, s2, sr) = (value1 as u64 & m, value2 as u64 & m, result as u64 & m);
        self.overflow = s1 == s2 && s1 != sr;
    }

    /// Carry is the bit just above `length` bytes of the wide result.
    pub fn set_carry(&mut self, value: u64, length: u32) {
        let carry_mask = sign_mask(length) << 1;
        self.carry = value & carry_mask != 0;
    }

    pub fn to_value(self) -> u32 {
        self.carry as u32 | (self.zero as u32) << 1 | (self.sign as u32) << 2 | (self.overflow as u32) << 3
    }

    pub fn from_value(&mut self, value: u32) {
        self.carry = value & 0x01 != 0;
        self.zero = value & 0x02 != 0;
        self.sign = value & 0x04 != 0;
        self.overflow = value & 0x08 != 0;
    }
}
