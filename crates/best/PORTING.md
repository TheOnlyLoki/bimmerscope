# Porting opcode handlers from C# to Rust

Source: `EdiabasLib/EdiabasLib/EdOperations.cs` in [ediabaslib](https://github.com/uholeschak/ediabaslib)
(handlers `private static void OpXxx(EdiabasNet ediabas, OpCode oc, Operand arg0, Operand arg1)`).
Helpers they call live in the same file or in `EdiabasNet.cs` next to it.

Each opcode group has one file in `src/ops/<group>.rs` with stub functions:

```rust
pub(crate) fn op_move(vm: &mut Vm, oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    Err(VmError::NotImplemented("move"))
}
```

Replace each stub body with a faithful port of the C# handler. Keep the
signature and name. Same behaviour, same edge cases, same flag updates.

## Rules

1. Only edit your own `src/ops/<group>.rs`. Never edit other files.
2. Port 1:1. Do not "improve" logic. Keep C# quirks; add a short comment if a quirk looks like a bug.
3. `EdValueType` is `u32`; C# `uint` math wraps, so use `wrapping_add`, `wrapping_sub`, `wrapping_mul`, `wrapping_shl`, etc.
   `EdFloatType` is `f64`. `Int64` results are `i64`.
4. Private C# helpers your handlers need (e.g. `GetArgsValueLength`): port them as private `fn`s in your file.
5. EdiabasNet fields not listed below: add them to `pub struct State` in your file and use `vm.st_<group>.field`.
6. Drop logging (`LogFormat`, `LogString`, `LogData`) and timing (`Stopwatch`, `_timeMeas`).
7. Add `#[cfg(test)] mod tests` at the end of your file with at least one `#[test]` per handler.
8. When done: `cargo build -p best` and `cargo test -p best` must pass with no warnings in your file.

## Mapping

| C# | Rust |
|---|---|
| `arg0.OpData1.GetType() != typeof(Register)` | `arg0.reg().is_none()` |
| `(Register)arg0.OpData1` | `arg0.reg().unwrap()` (after the check) |
| `(EdValueType)arg0.OpData1` | `arg0.imm()?` |
| `arg0.AddrMode == OpAddrMode.RegS` | `arg0.mode == AddrMode::RegS` |
| `arg0.GetDataType() == typeof(EdValueType)` | `arg0.data_type() == DataType::Value` |
| `arg0.GetDataType() == typeof(byte[])` | `arg0.data_type() == DataType::Array` |
| `arg0.GetDataType() == typeof(EdFloatType)` | `arg0.data_type() == DataType::Float` |
| `arg0.GetValueData()` | `vm.op_value(arg0, 0)?` |
| `arg0.GetValueData(len)` | `vm.op_value(arg0, len)?` |
| `arg0.GetRawData()` | `vm.op_raw(arg0)?` returns `Value::{Int(u32), Float(f64), Bytes(Vec<u8>)}` |
| `rawData is EdValueType` | `matches!(raw, Value::Int(_))` |
| `arg0.GetDataLen()` / `GetDataLen(true)` | `vm.op_data_len(arg0, false)?` / `vm.op_data_len(arg0, true)?` |
| `arg0.GetValueMask()` | port locally: 1 → `0xFF`, 2 → `0xFFFF`, 4 → `0xFFFF_FFFF` of `op_data_len` |
| `arg0.GetArrayData()` | `vm.op_array(arg0)?` (returns `Vec<u8>`) |
| `arg0.GetStringData()` | `vm.op_string(arg0)?` |
| `arg0.GetFloatData()` | `vm.op_float(arg0)?` |
| `arg0.SetRawData(value)` (uint) | `vm.op_set_raw(arg0, Value::Int(value), 1)?` |
| `arg0.SetRawData(value, len)` | `vm.op_set_raw(arg0, Value::Int(value), len)?` |
| `arg0.SetRawData(bytes)` | `vm.op_set_raw(arg0, Value::Bytes(bytes), 1)?` |
| `arg0.SetRawData(doubleValue)` | `vm.op_set_raw(arg0, Value::Float(d), 1)?` |
| `arg0.SetArrayData(bytes)` | `vm.op_set_array(arg0, bytes)?` |
| `arg0.SetStringData(s)` | `vm.op_set_string(arg0, &s)?` |
| `register.SetArrayData(bytes, keepLength)` | `vm.reg_set_array(reg, &bytes, keep_length)?` |
| `register.GetArrayData(true)` | `vm.regs.string(reg)?.get(true)` |
| `register.GetValueData()` / `SetValueData(v)` | `vm.regs.value(reg)?` / `vm.regs.set_value(reg, v)?` |
| `register.ClearData()` | `vm.regs.string_mut(reg)?.clear()` |
| `ediabas._flags.Carry` (Zero, Sign, Overflow) | `vm.flags.carry` (`zero`, `sign`, `overflow`) |
| `ediabas._flags.UpdateFlags(v, len)` | `vm.flags.update_flags(v, len)` |
| `ediabas._flags.SetOverflow(a, b, r, len)` | `vm.flags.set_overflow(a, b, r, len)` |
| `ediabas._flags.SetCarry(v64, len)` | `vm.flags.set_carry(v64, len)` |
| `ediabas._flags.ToValue()` / `FromValue(v)` | `vm.flags.to_value()` / `vm.flags.from_value(v)` |
| `ediabas.SetError(ErrorCodes.EDIABAS_X)` | `vm.set_error(ErrorCode::EDIABAS_X)?;` |
| `ediabas.RaiseError(ErrorCodes.EDIABAS_X)` | `return Err(vm.raise_error(ErrorCode::EDIABAS_X));` |
| `throw new ArgumentOutOfRangeException(.., "msg")` | `return internal("msg");` |
| `ediabas._pcCounter` | `vm.pc` (u32) |
| `ediabas._jobEnd = true` | `vm.job_end = true` |
| `ediabas._stackList.Push(b)` / `.Pop()` / `.Count` | `vm.stack.push(b)` / `vm.stack.pop()` (None → `return internal("stack empty")`) / `vm.stack.len()` |
| `ediabas._errorTrapBitNr` / `_errorTrapMask` | `vm.error_trap_bit_nr` (i32) / `vm.error_trap_mask` (u32) |
| `ediabas.ArrayMaxSize` / `ArrayMaxBufSize` | `vm.array_max_size()` / `vm.array_max_buf_size()` |
| `new ResultData(ResultType.TypeB, name, (Int64)v)` | `ResultData::new(ResultType::B, name, ResultValue::Int(v as i64))` |
| result string / byte[] / double | `ResultValue::Str(s)` / `ResultValue::Bytes(b)` / `ResultValue::Float(d)` |
| `ediabas.SetResultData(r)` / `SetSysResultData(r)` | `vm.set_result_data(r)` / `vm.set_sys_result_data(r)` |
| `ediabas._resultsRequestDict` "is this result wanted" checks | `vm.is_result_requested(name)` |
| `ediabas._resultDict` / `_resultSetsTemp` | `vm.results` / `vm.result_sets` (`ResultSet` has `set`, `get`, `clear`, `is_empty`) |
| `ediabas._resultJobStatus` | `vm.result_job_status` |
| `ediabas.GetActiveArgStrings()` | `vm.active_arg_strings()` (`Vec<String>`) |
| `ediabas.GetActiveArgBinary()` | `vm.active_arg_binary()` (`&[u8]`) |
| `ediabas.GetConfigProperty(k)` / `SetConfigProperty(k, v)` | `vm.config_property(k)` (`Option<&str>`) / `vm.config.insert(k.into(), v.into())` |
| `ediabas.EdInterfaceClass` | `vm.iface` (`Option<Box<dyn Interface>>`, see `src/iface.rs`) |
| interface call that returns false (it already set an error) | interface returns `Err(code)`: `if let Err(e) = r { vm.set_error(e)?; }` |
| `Encoding.GetString(bytes)` / `GetBytes(s)` | `crate::ENCODING.decode_without_bom_handling(&b).0` / `crate::ENCODING.encode(s).0` (Windows-1252) |
| `ediabas._requestInit` | `vm.request_init` |
| tables (`GetTableFs`, `SeekTable`, `GetTableEntry`, ...) | `vm.sgbd.tables` (`Vec<sgbd::Table>` with `name`, `columns`, `rows: Vec<Vec<String>>`), `vm.sgbd.table(name)` |

## Unit test template

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::regs::Reg;

    fn run(f: super::super::OpFn, a0: &mut Operand, a1: &mut Operand, vm: &mut Vm) -> VmResult<()> {
        f(vm, &super::super::OPCODES[0], a0, a1)
    }

    #[test]
    fn move_imm_to_reg_sets_zero_flag() {
        let mut vm = Vm::for_test();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm32(0));
        run(op_move, &mut a0, &mut a1, &mut vm).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);
        assert!(vm.flags.zero);
    }
}
```

Operand helpers: `Operand::r(Reg::B(0))`, `Operand::imm8/imm16/imm32(v)`,
`Operand::imm_str(b"..")`, `Operand::idx(Reg::S(0), 3)`, `Operand::none()`.
Registers: `Reg::B(0..32)`, `Reg::I(0..16)`, `Reg::L(0..8)`, `Reg::F(0..8)`, `Reg::S(0..16)`.
Set a string register: `vm.reg_set_array(Reg::S(0), b"abc\0", false).unwrap()`.
Errors: untrapped `set_error` returns `Err(VmError::Raised(code))`.
