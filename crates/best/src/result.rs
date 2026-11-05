//! Job results (C#: `ResultData`, `Dictionary<string, ResultData>`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultType {
    /// 8 bit unsigned
    B,
    /// 16 bit unsigned
    W,
    /// 32 bit unsigned
    D,
    /// 64 bit unsigned
    Q,
    /// 8 bit char (signed)
    C,
    /// 16 bit signed
    I,
    /// 32 bit signed
    L,
    /// 64 bit signed
    LL,
    /// float
    R,
    /// string
    S,
    /// byte array
    Y,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultValue {
    /// All integer types. C# stores them as `Int64`.
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResultData {
    pub ty: ResultType,
    pub name: String,
    pub value: ResultValue,
}

impl ResultData {
    pub fn new(ty: ResultType, name: impl Into<String>, value: ResultValue) -> Self {
        Self {
            ty,
            name: name.into(),
            value,
        }
    }
}

/// One result set, keyed case-insensitively by name, in insertion order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResultSet(pub Vec<ResultData>);

impl ResultSet {
    /// Inserts or replaces (C#: `SetResultData`).
    pub fn set(&mut self, data: ResultData) {
        match self
            .0
            .iter_mut()
            .find(|r| r.name.eq_ignore_ascii_case(&data.name))
        {
            Some(slot) => *slot = data,
            None => self.0.push(data),
        }
    }

    pub fn get(&self, name: &str) -> Option<&ResultData> {
        self.0.iter().find(|r| r.name.eq_ignore_ascii_case(name))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}
