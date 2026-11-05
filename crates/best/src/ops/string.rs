//! Opcodes: scmp, scat, scut, slen, spaste, serase, swap, setspc, srevrs, stoken, strcat, strcmp, strlen, ssize.
//! Ported from `EdOperations.cs`.

#![allow(unused_variables, unused_imports)]

use super::OpCode;
use crate::operand::{AddrMode, DataType, OpData, Operand, Value};
use crate::regs::Reg;
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult, internal};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug, Default)]
pub struct State {
    /// C#: `_tokenSeparator` (set by setspc, read by stoken). C# initial value: empty.
    pub token_separator: String,
    /// C#: `_tokenIndex` (EdValueType). C# initial value: 0.
    pub token_index: u32,
}

/// C#: `arg.OpData1.GetType() == typeof(Register)` check, returns the register.
fn reg_arg(arg: &Operand, name: &str) -> VmResult<Reg> {
    match arg.reg() {
        Some(r) => Ok(r),
        None => internal(format!("{name}: Invalid type")),
    }
}

/// C#: `(EdValueType)arg.OpData{n}` for an immediate value slot.
fn val_of(d: &OpData, name: &str) -> VmResult<u32> {
    match d {
        OpData::Val(v) => Ok(*v),
        _ => internal(format!("{name}: invalid data type")),
    }
}

/// C#: `startIdx` of OpSerase / OpSpaste. IdxImm keeps it as a value,
/// IdxReg as a register (`Register.GetValueData()`). Other modes throw.
fn index_start(vm: &Vm, arg0: &Operand, name: &str) -> VmResult<u32> {
    match (arg0.mode, &arg0.d2) {
        (AddrMode::IdxImm, OpData::Val(v)) => Ok(*v),
        (AddrMode::IdxReg, OpData::Reg(r)) => vm.regs.value(*r),
        (AddrMode::IdxImm | AddrMode::IdxReg, _) => {
            internal(format!("{name}: invalid index data type"))
        }
        _ => internal(format!("{name}: Invalid mode")),
    }
}

/// C#: `Flags.UpdateFlags` throws for lengths other than 1, 2, 4.
/// Returned as an internal error here instead of a panic.
fn update_flags(vm: &mut Vm, value: u32, len: u32) -> VmResult<()> {
    if !matches!(len, 1 | 2 | 4) {
        return internal("Flags.UpdateFlags: Invalid length");
    }
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpScmp` (scmp)
pub(crate) fn op_scmp(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let data1 = vm.op_array(arg0)?;
    let data2 = vm.op_array(arg1)?;
    vm.flags.zero = data1 == data2;
    Ok(())
}

/// C#: `OpScat` (scat)
pub(crate) fn op_scat(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpScat")?;
    let mut result = vm.op_array(arg0)?;
    let data2 = vm.op_array(arg1)?;
    result.extend_from_slice(&data2);
    if result.len() > vm.array_max_size() as usize {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0001);
    }
    vm.op_set_raw(arg0, Value::Bytes(result), 1)
}

/// C#: `OpScut` (scut)
pub(crate) fn op_scut(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpScut")?;
    let data = vm.op_array(arg0)?;
    let len = vm.op_value(arg1, 0)? as usize;
    // len includes the terminating 0. C# keeps the first (length - len) bytes,
    // so the cut is taken from the end of the used data.
    if len > data.len() {
        vm.op_set_array(arg0, Vec::new())
    } else {
        vm.op_set_array(arg0, data[..data.len() - len].to_vec())
    }
}

/// C#: `OpSlen` (slen)
pub(crate) fn op_slen(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpSlen")?;
    let len = vm.op_data_len(arg1, false)?;
    vm.op_set_raw(arg0, Value::Int(len), 1)?;
    vm.flags.overflow = false;
    let value = vm.op_value(arg0, 0)?;
    let arg0_len = vm.op_data_len(arg0, false)?;
    update_flags(vm, value, arg0_len)
}

/// C#: `OpSpaste` (spaste). Inserts arg1 into arg0 at the start index.
/// A start index inside the array but past the used length is not an error:
/// the C# code simply does nothing in that case.
pub(crate) fn op_spaste(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let reg = reg_arg(arg0, "OpSpaste")?;
    let start = index_start(vm, arg0, "OpSpaste")?;
    let dest = vm.regs.string(reg)?.get(false);
    let source = vm.op_array(arg1)?;

    if start >= vm.array_max_size() {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0001);
    }
    let start = start as usize;
    if start < dest.len() {
        let mut result = Vec::with_capacity(dest.len() + source.len());
        result.extend_from_slice(&dest[..start]);
        result.extend_from_slice(&source);
        result.extend_from_slice(&dest[start..]);
        if result.len() > vm.array_max_size() as usize {
            return vm.set_error(ErrorCode::EDIABAS_BIP_0001);
        }
        vm.reg_set_array(reg, &result, false)?;
    }
    Ok(())
}

/// C#: `OpSerase` (serase). Removes `arg1` bytes starting at the start index.
pub(crate) fn op_serase(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let reg = reg_arg(arg0, "OpSerase")?;
    let start = index_start(vm, arg0, "OpSerase")?;
    let data = vm.regs.string(reg)?.get(false);
    let len = vm.op_value(arg1, 0)?;
    // C#: `startIdx + len` is uint arithmetic and wraps; the comparison
    // with the int index is done in long.
    let end = start.wrapping_add(len) as i64;
    let start = start as i64;
    let result: Vec<u8> = data
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let i = *i as i64;
            i < start || i >= end
        })
        .map(|(_, &b)| b)
        .collect();
    vm.reg_set_array(reg, &result, false)
}

/// C#: `OpSwap` (swap). Reverses `dataLen` bytes of the full buffer at `startIdx`.
pub(crate) fn op_swap(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let reg = reg_arg(arg0, "OpSwap")?;
    let start = val_of(&arg0.d2, "OpSwap")?;
    let data_len = val_of(&arg0.d3, "OpSwap")?;
    let mut buf = vm.regs.string(reg)?.get(true);

    // C#: uint addition wraps.
    if start.wrapping_add(data_len) > vm.array_max_size() {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0001);
    }
    let (start, data_len) = (start as usize, data_len as usize);
    if start + data_len > buf.len() {
        // C#: Array.Reverse throws ArgumentException here.
        return internal("OpSwap: Array.Reverse range invalid");
    }
    buf[start..start + data_len].reverse();
    vm.reg_set_array(reg, &buf, true)
}

/// C#: `OpSetspc` (setspc). Stores the token separator and index for stoken.
pub(crate) fn op_setspc(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    // C# assigns the separator before evaluating arg1, keep that order.
    let separator = vm.op_string(arg0)?;
    vm.st_string.token_separator = separator;
    let index = vm.op_value(arg1, 0)?;
    vm.st_string.token_index = index;
    Ok(())
}

/// C#: `OpSrevrs` (srevrs). Reverses the used bytes of arg0.
pub(crate) fn op_srevrs(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    // C# error text says "OpSpaste" here (copy/paste). Kept as the message only.
    reg_arg(arg0, "OpSpaste")?;
    let mut data = vm.op_array(arg0)?;
    data.reverse();
    vm.op_set_array(arg0, data)
}

/// C#: `OpStoken` (stoken). Stores token number `_tokenIndex` (1-based) of arg1,
/// split at any character of the separator. Zero flag set when no token.
pub(crate) fn op_stoken(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpStoken")?;
    if vm.st_string.token_separator.is_empty() {
        vm.flags.zero = true;
        return Ok(());
    }
    let separators: Vec<char> = vm.st_string.token_separator.chars().collect();
    let index = vm.st_string.token_index;
    let split_string = vm.op_string(arg1)?;
    // C#: String.Split(char[]) keeps empty entries.
    let words: Vec<&str> = split_string.split(|c| separators.contains(&c)).collect();
    if index < 1 || index as usize > words.len() {
        vm.flags.zero = true;
    } else {
        let word = words[index as usize - 1].to_string();
        vm.op_set_string(arg0, &word)?;
        vm.flags.zero = false;
    }
    Ok(())
}

/// C#: `OpStrcat` (strcat). Appends arg1 to arg0; arg1 is cut to fit `ArrayMaxSize`.
pub(crate) fn op_strcat(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpStrcat")?;
    let len1 = vm.op_data_len(arg0, false)?;
    let string1 = vm.op_string(arg0)?;
    let mut string2 = vm.op_string(arg1)?;

    let max = vm.array_max_size();
    let string2_len = string2.chars().count() as u64;
    if len1 as u64 + string2_len > max as u64 {
        // C#: Substring(0, (int)(ArrayMaxSize - len1)); a negative count throws.
        if len1 > max {
            return internal("OpStrcat: Substring length invalid");
        }
        string2 = string2.chars().take((max - len1) as usize).collect();
    }
    let result = format!("{string1}{string2}");
    vm.op_set_string(arg0, &result)
}

/// C#: `OpStrcmp` (strcmp). Zero flag set when the strings differ.
pub(crate) fn op_strcmp(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let string1 = vm.op_string(arg0)?;
    let string2 = vm.op_string(arg1)?;
    vm.flags.zero = string1 != string2;
    Ok(())
}

/// C#: `OpStrlen` (strlen). Stores the string length of arg1 in arg0.
pub(crate) fn op_strlen(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpStrlen")?;
    let result = vm.op_string(arg1)?.chars().count() as u32;
    vm.op_set_raw(arg0, Value::Int(result), 1)?;
    vm.flags.overflow = false;
    let arg0_len = vm.op_data_len(arg0, false)?;
    update_flags(vm, result, arg0_len)
}

/// C#: `OpSsize` (ssize). Stores the full buffer size (`ArrayMaxBufSize`) in arg0.
pub(crate) fn op_ssize(
    vm: &mut Vm,
    oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    reg_arg(arg0, "OpSsize")?;
    let size = vm.array_max_buf_size();
    vm.op_set_raw(arg0, Value::Int(size), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{OPCODES, OpFn};

    fn run(f: OpFn, a0: &mut Operand, a1: &mut Operand, vm: &mut Vm) -> VmResult<()> {
        f(vm, &OPCODES[0], a0, a1)
    }

    fn set_s(vm: &mut Vm, i: u8, data: &[u8]) {
        vm.reg_set_array(Reg::S(i), data, false).unwrap();
    }

    fn get_s(vm: &Vm, i: u8) -> Vec<u8> {
        vm.regs.string(Reg::S(i)).unwrap().get(false)
    }

    #[test]
    fn scmp_zero_on_equal_arrays() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abc");
        set_s(&mut vm, 1, b"abc");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::r(Reg::S(1)));
        run(op_scmp, &mut a0, &mut a1, &mut vm).unwrap();
        assert!(vm.flags.zero);
        set_s(&mut vm, 1, b"abd");
        run(op_scmp, &mut a0, &mut a1, &mut vm).unwrap();
        assert!(!vm.flags.zero);
    }

    #[test]
    fn scat_appends_and_checks_max_size() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"ab");
        set_s(&mut vm, 1, b"cd");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::r(Reg::S(1)));
        run(op_scat, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"abcd");

        set_s(&mut vm, 0, &vec![b'a'; 1000]);
        set_s(&mut vm, 1, &[b'b'; 30]);
        let res = run(op_scat, &mut a0, &mut a1, &mut vm);
        assert!(matches!(
            res,
            Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0001))
        ));
    }

    #[test]
    fn scut_removes_from_end() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abc\0");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm8(1));
        run(op_scut, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"abc");

        let mut a1 = Operand::imm8(10);
        run(op_scut, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"");
    }

    #[test]
    fn slen_stores_length_of_arg1() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 1, b"hello");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::r(Reg::S(1)));
        run(op_slen, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 5);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn spaste_inserts_at_index() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abcdef");
        let (mut a0, mut a1) = (Operand::idx(Reg::S(0), 2), Operand::imm_str(b"XY"));
        run(op_spaste, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"abXYcdef");

        // Index past the used length: nothing happens, no error.
        let mut a0 = Operand::idx(Reg::S(0), 20);
        run(op_spaste, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"abXYcdef");
    }

    #[test]
    fn serase_removes_range() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abcdef");
        let (mut a0, mut a1) = (Operand::idx(Reg::S(0), 1), Operand::imm8(2));
        run(op_serase, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"adef");
    }

    #[test]
    fn swap_reverses_range_and_keeps_length() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abcdef");
        let mut a0 = Operand::new(
            AddrMode::IdxImmLenImm,
            OpData::Reg(Reg::S(0)),
            OpData::Val(1),
            OpData::Val(3),
        );
        let mut a1 = Operand::none();
        run(op_swap, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"adcbef");
    }

    #[test]
    fn setspc_stoken_selects_token() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::imm_str(b",;"), Operand::imm8(2));
        run(op_setspc, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(vm.st_string.token_separator, ",;");
        assert_eq!(vm.st_string.token_index, 2);

        let (mut s1, mut src) = (Operand::r(Reg::S(1)), Operand::imm_str(b"a,bb;ccc"));
        run(op_stoken, &mut s1, &mut src, &mut vm).unwrap();
        assert!(!vm.flags.zero);
        assert_eq!(vm.op_string(&s1).unwrap(), "bb");

        // Index out of range: zero flag set, register unchanged.
        vm.st_string.token_index = 4;
        run(op_stoken, &mut s1, &mut src, &mut vm).unwrap();
        assert!(vm.flags.zero);
        assert_eq!(vm.op_string(&s1).unwrap(), "bb");

        // Empty separator: zero flag set.
        vm.st_string.token_separator.clear();
        run(op_stoken, &mut s1, &mut src, &mut vm).unwrap();
        assert!(vm.flags.zero);
    }

    #[test]
    fn srevrs_reverses_bytes() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abc");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::none());
        run(op_srevrs, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(get_s(&vm, 0), b"cba");
    }

    #[test]
    fn strcat_appends_with_nul() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"ab");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm_str(b"cd"));
        run(op_strcat, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(vm.op_string(&a0).unwrap(), "abcd");
        assert_eq!(get_s(&vm, 0), b"abcd\0");
    }

    #[test]
    fn strcat_cuts_arg1_to_max_size() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, &vec![b'a'; 1000]);
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm_str(&[b'b'; 100]));
        run(op_strcat, &mut a0, &mut a1, &mut vm).unwrap();
        // ArrayMaxSize is 1023; the result is 1023 chars plus the NUL.
        assert_eq!(vm.op_string(&a0).unwrap().len(), 1023);
    }

    #[test]
    fn strcmp_zero_when_different() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 0, b"abc");
        set_s(&mut vm, 1, b"abc");
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::r(Reg::S(1)));
        run(op_strcmp, &mut a0, &mut a1, &mut vm).unwrap();
        assert!(!vm.flags.zero);
        set_s(&mut vm, 1, b"abd");
        run(op_strcmp, &mut a0, &mut a1, &mut vm).unwrap();
        assert!(vm.flags.zero);
    }

    #[test]
    fn strlen_stores_length() {
        let mut vm = Vm::for_test();
        set_s(&mut vm, 1, b"hello");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::r(Reg::S(1)));
        run(op_strlen, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 5);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn ssize_stores_buffer_size() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::none());
        run(op_ssize, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), vm.array_max_buf_size());
    }
}
