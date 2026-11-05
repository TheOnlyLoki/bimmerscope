//! BEST/2 interpreter: runs EDIABAS jobs from SGBD bytecode.
//!
//! Ported from EdiabasLib (`EdiabasNet.cs`, `EdOperations.cs`, GPL-3.0).
//! Names follow the C# source so both can be read side by side.

pub mod error_code;
pub mod flags;
pub mod iface;
pub mod job;
pub mod operand;
pub mod ops;
pub mod regs;
pub mod result;
pub mod sim;
pub mod telegram;
pub mod vm;

pub use error_code::ErrorCode;
pub use iface::Interface;
pub use job::Ediabas;
pub use result::{ResultData, ResultSet, ResultType, ResultValue};
pub use vm::Vm;

/// Windows-1252, the text encoding of SGBD files and BEST/2 strings.
pub(crate) const ENCODING: &encoding_rs::Encoding = encoding_rs::WINDOWS_1252;

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    /// `SetError` hit an untrapped error (C#: `EdiabasNetException`).
    #[error("EDIABAS error {0:?}")]
    Raised(ErrorCode),
    /// Malformed bytecode or operand types (C#: `ArgumentOutOfRangeException`).
    #[error("internal: {0}")]
    Internal(String),
    #[error("opcode not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("job not found: {0}")]
    JobNotFound(String),
    /// SGBD file missing or unreadable, or group variant not found
    /// (C#: `ArgumentOutOfRangeException` in `ResolveSgbdFile`/`OpenSgbdFs`).
    #[error("file: {0}")]
    File(String),
    #[error("sgbd: {0}")]
    Sgbd(#[from] sgbd::Error),
}

pub type VmResult<T> = Result<T, VmError>;

/// Shorthand for an internal error, like the C# `throw new ArgumentOutOfRangeException(...)`.
pub(crate) fn internal<T>(msg: impl Into<String>) -> VmResult<T> {
    Err(VmError::Internal(msg.into()))
}
