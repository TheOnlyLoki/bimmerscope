//! Opcodes: gettmr, settmr, sett, clrt, break, fclose, fopen, fread, freadln, fseek, fseekln, ftell, ftellln, wait, date, time, cfgig, cfgsg, cfgis, shmset, shmget, iupdate, irange, iincpos, ticks, waitex.
//! Ported from `EdOperations.cs`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::OpCode;
use crate::operand::{Operand, Value};
use crate::vm::Vm;
use crate::{ENCODING, ErrorCode, VmResult, internal};

/// C#: `MaxFiles`. Number of user file slots; the slot index is the handle.
const MAX_USER_FILES: usize = 5;

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug)]
pub struct State {
    /// C#: `_infoProgressRange`. -1 = unset.
    pub info_progress_range: i64,
    /// C#: `_infoProgressPos`. -1 = unset.
    pub info_progress_pos: i64,
    /// C#: `_infoProgressText`.
    pub info_progress_text: String,
    /// C#: `_userFilesArray`.
    user_files: [Option<BufReader<File>>; MAX_USER_FILES],
}

impl Default for State {
    fn default() -> Self {
        Self {
            info_progress_range: -1,
            info_progress_pos: -1,
            info_progress_text: String::new(),
            user_files: std::array::from_fn(|_| None),
        }
    }
}

impl State {
    /// C#: `GetUserFile(index)`.
    fn user_file_mut(&mut self, index: u32) -> Option<&mut BufReader<File>> {
        self.user_files.get_mut(index as usize)?.as_mut()
    }

    /// C#: `StoreUserFile`. Returns the new handle, or None if all slots are used.
    fn store_user_file(&mut self, file: BufReader<File>) -> Option<usize> {
        let index = self.user_files.iter().position(|f| f.is_none())?;
        self.user_files[index] = Some(file);
        Some(index)
    }

    /// C#: `CloseUserFile`. False for an invalid handle.
    fn close_user_file(&mut self, index: u32) -> bool {
        match self.user_files.get_mut(index as usize) {
            Some(slot) => {
                *slot = None;
                true
            }
            None => false,
        }
    }
}

/// C#: `SharedDataDict`. Static, shared by all VM instances. Keys are upper case.
static SHARED_DATA: LazyLock<Mutex<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn shared_data() -> MutexGuard<'static, HashMap<String, Vec<u8>>> {
    SHARED_DATA.lock().unwrap_or_else(|e| e.into_inner())
}

/// C#: `OpGettmr` (gettmr)
pub(crate) fn op_gettmr(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpGettmr: Invalid type");
    }
    let mask = vm.error_trap_mask;
    vm.op_set_raw(arg0, Value::Int(mask), 1)?;
    let value = vm.op_value(arg0, 0)?;
    let len = vm.op_data_len(arg0, false)?;
    vm.flags.update_flags(value, len);
    Ok(())
}

/// C#: `OpSettmr` (settmr)
pub(crate) fn op_settmr(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    vm.error_trap_mask = vm.op_value(arg0, 0)?;
    Ok(())
}

/// C#: `OpSett` (sett)
pub(crate) fn op_sett(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let mut error = vm.op_value(arg0, 0)?;
    if error == 0 {
        // C# quirk: 0 selects the bit above the 32 error codes instead of bit 0.
        error = 0x4000_0000;
    }
    // C# `(int)error`: wrapping cast.
    vm.error_trap_bit_nr = error as i32;
    Ok(())
}

/// C#: `OpClrt` (clrt)
pub(crate) fn op_clrt(
    vm: &mut Vm,
    _oc: &OpCode,
    _arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    vm.error_trap_bit_nr = -1;
    Ok(())
}

/// C#: `OpBreak` (break). User break: raises `EDIABAS_BIP_0008`.
pub(crate) fn op_break(
    vm: &mut Vm,
    _oc: &OpCode,
    _arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    vm.set_error(ErrorCode::EDIABAS_BIP_0008)
}

/// C#: `OpFclose` (fclose)
pub(crate) fn op_fclose(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let handle = vm.op_value(arg0, 1)?;
    if !vm.st_misc.close_user_file(handle) {
        vm.set_error(ErrorCode::EDIABAS_BIP_0006)?;
    }
    Ok(())
}

/// C#: `OpFopen` (fopen). Handle is written to arg0, -1 (0xFFFFFFFF) on failure.
pub(crate) fn op_fopen(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFopen: Invalid type");
    }
    let mut handle: i32 = -1;

    let file_name = vm.op_string(arg1)?;
    // C# `File.Exists`: true for files only, not directories.
    if !Path::new(&file_name).is_file() {
        vm.set_error(ErrorCode::EDIABAS_BIP_0006)?;
    } else {
        // C# `MemoryStreamReader.OpenRead` with file name encoding is not ported.
        match File::open(&file_name) {
            Ok(file) => match vm.st_misc.store_user_file(BufReader::new(file)) {
                Some(index) => handle = index as i32,
                None => vm.set_error(ErrorCode::EDIABAS_BIP_0006)?,
            },
            Err(_) => vm.set_error(ErrorCode::EDIABAS_BIP_0006)?,
        }
    }
    vm.op_set_raw(arg0, Value::Int(handle as u32), 1)?;
    vm.flags.update_flags(handle as u32, 1);
    Ok(())
}

/// C#: `OpFread` (fread). One byte; 0 with carry set at end of file or on error.
pub(crate) fn op_fread(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFread: Invalid type");
    }
    let handle = vm.op_value(arg1, 1)?;
    let value: i32 = match with_user_file(vm, handle, read_byte)? {
        Some(Some(b)) => b as i32,
        _ => -1,
    };
    let value = if value < 0 {
        vm.flags.carry = true;
        0
    } else {
        vm.flags.carry = false;
        value
    };
    vm.op_set_raw(arg0, Value::Int(value as u32), 1)
}

/// C#: `OpFreadln` (freadln). Line without line break; empty with carry set at end of file or on error.
pub(crate) fn op_freadln(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFreadln: Invalid type");
    }
    let handle = vm.op_value(arg1, 1)?;
    let line = with_user_file(vm, handle, read_file_line)?.flatten();
    let bytes = match line {
        Some(line) => {
            vm.flags.carry = false;
            line
        }
        None => {
            vm.flags.carry = true;
            Vec::new()
        }
    };
    vm.op_set_array(arg0, bytes)
}

/// C#: `OpFseek` (fseek)
pub(crate) fn op_fseek(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let handle = vm.op_value(arg0, 1)?;
    let position = vm.op_value(arg1, 0)?;
    with_user_file(vm, handle, |f| f.seek(SeekFrom::Start(position as u64)))?;
    Ok(())
}

/// C#: `OpFseekln` (fseekln). Seeks to the start of line `line` (0-based).
pub(crate) fn op_fseekln(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let handle = vm.op_value(arg0, 1)?;
    let line = vm.op_value(arg1, 0)?;
    with_user_file(vm, handle, |f| {
        f.seek(SeekFrom::Start(0))?;
        for _ in 0..line {
            if read_file_line_length(f)?.is_none() {
                break;
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// C#: `OpFtell` (ftell). Byte position (truncated to 32 bits, as C# `(Int32)`).
pub(crate) fn op_ftell(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFtell: Invalid type");
    }
    let handle = vm.op_value(arg1, 1)?;
    let position = with_user_file(vm, handle, |f| f.stream_position())?.map_or(0, |p| p as u32);
    vm.op_set_raw(arg0, Value::Int(position), 1)?;
    vm.flags.update_flags(position, 4);
    Ok(())
}

/// C#: `OpFtellln` (ftellln). Index of the line the current position is in (0-based).
pub(crate) fn op_ftellln(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpFtellln: Invalid type");
    }
    let handle = vm.op_value(arg1, 1)?;
    let line = with_user_file(vm, handle, line_index_at_position)?.unwrap_or(0);
    vm.op_set_raw(arg0, Value::Int(line), 1)?;
    vm.flags.update_flags(line, 4);
    Ok(())
}

/// C#: `OpWait` (wait). Seconds.
pub(crate) fn op_wait(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let value = vm.op_value(arg0, 0)?;
    // C#: `(int)(uint * 1000)`, the product is long, then truncated to int.
    sleep_ms((value as i64 * 1000) as i32)
}

/// C#: `OpWaitex` (waitex). Milliseconds.
pub(crate) fn op_waitex(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let value = vm.op_value(arg0, 0)?;
    sleep_ms(value as i32)
}

/// C#: `OpTicks` (ticks). Milliseconds since 0001-01-01, truncated to 32 bits.
pub(crate) fn op_ticks(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpTicks: Invalid type");
    }
    // UTC, C# uses local time (DateTime.Now). No timezone support without a new crate.
    let value = ticks_ms_since_0001(unix_now().as_millis() as i64);
    vm.op_set_raw(arg0, Value::Int(value), 1)
}

/// C#: `OpDate` (date). Writes `[day, month, year % 100, week, weekday]`.
pub(crate) fn op_date(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpDate: Invalid type");
    }
    // UTC, C# uses local time (DateTime.Now). No timezone support without a new crate.
    let secs = unix_now().as_secs() as i64;
    vm.op_set_array(arg0, date_bytes(secs).to_vec())
}

/// C#: `OpTime` (time). Writes `[hour, minute, second]`.
pub(crate) fn op_time(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpTime: Invalid type");
    }
    // UTC, C# uses local time (DateTime.Now). No timezone support without a new crate.
    let secs = unix_now().as_secs() as i64;
    vm.op_set_array(arg0, time_bytes(secs).to_vec())
}

/// C#: `OpCfgig` (cfgig). Integer config property into arg0, unchanged if unset.
pub(crate) fn op_cfgig(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpCfgig: Invalid type");
    }
    let key = vm.op_string(arg1)?;
    if let Some(value) = vm.config_property(&key).map(str::to_owned) {
        // C#: `(EdValueType)StringToValue(value)`, wrapping cast.
        let v = string_to_value(&value) as u32;
        vm.op_set_raw(arg0, Value::Int(v), 1)?;
    }
    Ok(())
}

/// C#: `OpCfgis` (cfgis). Stores arg1 (decimal) as config property named arg0.
pub(crate) fn op_cfgis(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let value = vm.op_value(arg1, 0)?;
    let key = vm.op_string(arg0)?;
    set_config_property(vm, &key, &value.to_string());
    Ok(())
}

/// C#: `OpCfgsg` (cfgsg). String config property into arg0, unchanged if unset.
pub(crate) fn op_cfgsg(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpCfgsg: Invalid type");
    }
    let key = vm.op_string(arg1)?;
    if let Some(value) = vm.config_property(&key).map(str::to_owned) {
        let bytes = ENCODING.encode(&value).0.into_owned();
        vm.op_set_array(arg0, bytes)?;
    }
    Ok(())
}

/// C#: `OpShmset` (shmset). Stores arg1 bytes under key arg0.
pub(crate) fn op_shmset(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    let key = vm.op_string(arg0)?.to_uppercase();
    let data = vm.op_array(arg1)?;
    shared_data().insert(key, data);
    Ok(())
}

/// C#: `OpShmget` (shmget). Carry set if the key is unknown (arg0 becomes empty).
pub(crate) fn op_shmget(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    arg1: &mut Operand,
) -> VmResult<()> {
    if arg0.reg().is_none() {
        return internal("OpShmget: Invalid type");
    }
    let key = vm.op_string(arg1)?.to_uppercase();
    let found = shared_data().get(&key).cloned();
    let data = match found {
        Some(data) => {
            vm.flags.carry = false;
            data
        }
        None => {
            vm.flags.carry = true;
            Vec::new()
        }
    };
    vm.op_set_array(arg0, data)
}

/// C#: `OpIupdate` (iupdate). Progress text.
pub(crate) fn op_iupdate(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let text = vm.op_string(arg0)?;
    vm.st_misc.info_progress_text = text;
    // C# `JobProgressInform()` calls a delegate. No progress callback in the Rust VM.
    Ok(())
}

/// C#: `OpIrange` (irange). Sets progress range, resets position.
pub(crate) fn op_irange(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let range = vm.op_value(arg0, 0)?;
    vm.st_misc.info_progress_pos = -1;
    vm.st_misc.info_progress_range = range as i64;
    // C# `JobProgressInform()`: no callback in the Rust VM.
    Ok(())
}

/// C#: `OpIincpos` (iincpos). Adds to the progress position, clamped to the range.
pub(crate) fn op_iincpos(
    vm: &mut Vm,
    _oc: &OpCode,
    arg0: &mut Operand,
    _arg1: &mut Operand,
) -> VmResult<()> {
    let inc = vm.op_value(arg0, 0)? as i64;
    let st = &mut vm.st_misc;
    let mut new_value = if st.info_progress_pos < 0 {
        inc
    } else {
        st.info_progress_pos.wrapping_add(inc)
    };
    if new_value > st.info_progress_range {
        new_value = st.info_progress_range;
    }
    st.info_progress_pos = new_value;
    // C# `JobProgressInform()`: no callback in the Rust VM.
    Ok(())
}

/// C#: `Thread.Sleep`. Negative values throw in C# (except -1 = infinite, not ported).
fn sleep_ms(ms: i32) -> VmResult<()> {
    if ms < 0 {
        return internal("Thread.Sleep: negative time");
    }
    thread::sleep(Duration::from_millis(ms as u64));
    Ok(())
}

/// Runs `op` on user file `handle`. A missing handle or an I/O error sets
/// `EDIABAS_BIP_0006` (C#: `SetError`) and returns `None`.
fn with_user_file<T>(
    vm: &mut Vm,
    handle: u32,
    op: impl FnOnce(&mut BufReader<File>) -> io::Result<T>,
) -> VmResult<Option<T>> {
    let result = vm.st_misc.user_file_mut(handle).map(op);
    match result {
        Some(Ok(v)) => Ok(Some(v)),
        _ => {
            vm.set_error(ErrorCode::EDIABAS_BIP_0006)?;
            Ok(None)
        }
    }
}

/// C#: `ReadByte`. `None` at end of file.
fn read_byte(f: &mut impl Read) -> io::Result<Option<u8>> {
    let mut b = [0u8; 1];
    Ok(if f.read(&mut b)? == 0 {
        None
    } else {
        Some(b[0])
    })
}

/// C#: after a `\r`, step back over the next byte unless it is `\n` (or EOF).
fn skip_line_feed(f: &mut BufReader<File>) -> io::Result<()> {
    match read_byte(f)? {
        Some(b'\n') | None => Ok(()),
        Some(_) => f.seek_relative(-1),
    }
}

/// C#: `ReadFileLine`. Returns `None` at end of file. C# quirk: a last line
/// without line break also returns `None` (its bytes are dropped).
fn read_file_line(f: &mut BufReader<File>) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let terminator = loop {
        match read_byte(f)? {
            None => return Ok(None),
            Some(b) if b == b'\r' || b == b'\n' => break b,
            Some(b) => line.push(b),
        }
    };
    if terminator == b'\r' {
        skip_line_feed(f)?;
    }
    Ok(Some(line))
}

/// C#: `ReadFileLineLength`. Returns `None` at end of file (same quirk as `read_file_line`).
fn read_file_line_length(f: &mut BufReader<File>) -> io::Result<Option<u64>> {
    let mut len = 0u64;
    let terminator = loop {
        match read_byte(f)? {
            None => return Ok(None),
            Some(b) if b == b'\r' || b == b'\n' => break b,
            Some(_) => len += 1,
        }
    };
    if terminator == b'\r' {
        skip_line_feed(f)?;
    }
    Ok(Some(len))
}

/// C#: `OpFtellln` body. Counts lines from the file start up to the current position.
fn line_index_at_position(f: &mut BufReader<File>) -> io::Result<u32> {
    let current = f.stream_position()?;
    f.seek(SeekFrom::Start(0))?;
    let mut line: u32 = 0;
    loop {
        if read_file_line_length(f)?.is_none() {
            break;
        }
        let pos = f.stream_position()?;
        if pos >= current {
            if pos == current {
                line = line.wrapping_add(1);
            }
            break;
        }
        line = line.wrapping_add(1);
    }
    f.seek(SeekFrom::Start(current))?;
    Ok(line)
}

/// C#: `SetConfigProperty` for cfgis. Empty value removes an existing key.
fn set_config_property(vm: &mut Vm, name: &str, value: &str) {
    let key = name.to_uppercase();
    // C# dictionary keys are upper case; match case-insensitively.
    let existing = vm
        .config
        .keys()
        .find(|k| k.eq_ignore_ascii_case(&key))
        .cloned();
    match existing {
        Some(k) => {
            if value.is_empty() {
                vm.config.remove(&k);
            } else {
                vm.config.insert(k, value.to_string());
            }
        }
        None => {
            vm.config.insert(key, value.to_string());
        }
    }
}

/// C#: `EdiabasNet.StringToValue`. `0x` hex, `0y` binary, else decimal cut at
/// `.` or `,`. Invalid input gives 0.
fn string_to_value(number: &str) -> i64 {
    let local = number.trim_end();
    if local.is_empty() {
        return 0;
    }
    let lower = local.to_lowercase();
    if let Some(rest) = lower.strip_prefix("0x") {
        // C# checks the first digit only, then `Convert.ToInt64(.., 16)`.
        let first_ok = rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if first_ok {
            return u64::from_str_radix(&local[2..], 16)
                .map(|v| v as i64)
                .unwrap_or(0);
        }
        return 0;
    }
    if lower.starts_with("0y") {
        return u64::from_str_radix(&local[2..], 2)
            .map(|v| v as i64)
            .unwrap_or(0);
    }
    if lower == "-" || lower == "--" {
        return 0;
    }
    if lower.chars().next().is_some_and(char::is_alphabetic) {
        return 0;
    }
    let conv = local.trim_start();
    let conv = match conv.find(['.', ',']) {
        Some(i) => &conv[..i],
        None => conv,
    };
    conv.trim().parse::<i64>().unwrap_or(0)
}

/// Current time since the Unix epoch (UTC).
fn unix_now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

/// C#: `DateTime.Now.Ticks / 10000`: milliseconds since 0001-01-01 (low 32 bits).
fn ticks_ms_since_0001(unix_ms: i64) -> u32 {
    (unix_ms + 62_135_596_800_000) as u32
}

/// Civil (year, month, day) from days since 1970-01-01, proleptic Gregorian.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Days since 1970-01-01 for a civil date.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let (m, d) = (m as i64, d as i64);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Week of year, Monday first, week 1 contains the first Thursday (ISO 8601).
/// Assumed equal to C# `GetWeekOfYear(FirstFourDayWeek, Monday)`; not checked against .NET.
fn iso_week(days: i64) -> u32 {
    let weekday = (days + 3).rem_euclid(7) + 1; // Mon = 1 .. Sun = 7
    let thursday = days - weekday + 4;
    let (year, _, _) = civil_from_days(thursday);
    ((thursday - days_from_civil(year, 1, 1)) / 7 + 1) as u32
}

/// C# `OpDate` payload: day, month, year % 100, week, weekday (Sunday = 7).
fn date_bytes(unix_secs: i64) -> [u8; 5] {
    let days = unix_secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let weekday = (days + 3).rem_euclid(7) + 1;
    [
        day as u8,
        month as u8,
        year.rem_euclid(100) as u8,
        iso_week(days) as u8,
        weekday as u8,
    ]
}

/// C# `OpTime` payload: hour, minute, second.
fn time_bytes(unix_secs: i64) -> [u8; 3] {
    let s = unix_secs.rem_euclid(86_400);
    [(s / 3600) as u8, (s % 3600 / 60) as u8, (s % 60) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VmError;
    use crate::regs::Reg;

    fn oc() -> &'static OpCode {
        &super::super::OPCODES[0]
    }

    /// VM with the default string buffer size (C#: 1024).
    fn test_vm() -> Vm {
        let mut vm = Vm::for_test();
        vm.set_array_max_buf_size(1024);
        vm
    }

    /// Writes `content` to a unique temp file for this test.
    fn temp_file(name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "best_misc_test_{}_{}.txt",
            name,
            std::process::id()
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    fn path_operand(path: &Path) -> Operand {
        Operand::imm_str(path.to_string_lossy().as_bytes())
    }

    #[test]
    fn gettmr_reads_trap_mask_and_sets_flags() {
        let mut vm = test_vm();
        vm.error_trap_mask = 0x1234;
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::none());
        op_gettmr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0x1234);
        assert!(!vm.flags.zero);

        // Byte register: low byte of 0x100 is 0, so zero flag set.
        vm.error_trap_mask = 0x100;
        let (mut a0, mut a1) = (Operand::r(Reg::B(0)), Operand::none());
        op_gettmr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.zero);
    }

    #[test]
    fn settmr_sets_trap_mask() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm32(0xABCD), Operand::none());
        op_settmr(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.error_trap_mask, 0xABCD);
    }

    #[test]
    fn sett_sets_trap_bit_and_zero_maps_to_0x40000000() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm8(5), Operand::none());
        op_sett(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.error_trap_bit_nr, 5);

        let (mut a0, mut a1) = (Operand::imm32(0), Operand::none());
        op_sett(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.error_trap_bit_nr, 0x4000_0000);
    }

    #[test]
    fn clrt_clears_trap_bit() {
        let mut vm = test_vm();
        vm.error_trap_bit_nr = 7;
        let (mut a0, mut a1) = (Operand::none(), Operand::none());
        op_clrt(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.error_trap_bit_nr, -1);
    }

    #[test]
    fn break_raises_bip_0008() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::none(), Operand::none());
        let r = op_break(&mut vm, oc(), &mut a0, &mut a1);
        assert!(matches!(
            r,
            Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0008))
        ));
    }

    #[test]
    fn wait_zero_ok_and_negative_errors() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm8(0), Operand::none());
        op_wait(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        // 3_000_000 s: 3e9 ms truncated to i32 is negative, C# Thread.Sleep throws.
        let (mut a0, mut a1) = (Operand::imm32(3_000_000), Operand::none());
        assert!(op_wait(&mut vm, oc(), &mut a0, &mut a1).is_err());
    }

    #[test]
    fn waitex_sleeps_milliseconds() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm16(1), Operand::none());
        op_waitex(&mut vm, oc(), &mut a0, &mut a1).unwrap();
    }

    #[test]
    fn ticks_is_ms_since_year_1() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::none());
        op_ticks(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let v = vm.regs.value(Reg::L(0)).unwrap();
        let now = ticks_ms_since_0001(unix_now().as_millis() as i64);
        assert!(v.wrapping_sub(now) < 10_000);
        assert_eq!(ticks_ms_since_0001(0), 62_135_596_800_000u64 as u32);
    }

    #[test]
    fn date_bytes_known_dates() {
        // 1970-01-01 is a Thursday, ISO week 1.
        assert_eq!(date_bytes(0), [1, 1, 70, 1, 4]);
        // 2021-01-01 (Friday) belongs to ISO week 53 of 2020.
        assert_eq!(date_bytes(18_628 * 86_400), [1, 1, 21, 53, 5]);
        // 2024-12-30 (Monday) is ISO week 1 of 2025.
        assert_eq!(date_bytes(20_087 * 86_400)[3], 1);
        // Sunday is weekday 7 (C#: 0 mapped to 7). 1970-01-04 is a Sunday.
        assert_eq!(date_bytes(3 * 86_400)[4], 7);
    }

    #[test]
    fn date_writes_five_bytes() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::none());
        op_date(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false).len(), 5);
    }

    #[test]
    fn time_bytes_hms() {
        assert_eq!(time_bytes(3723), [1, 2, 3]);
        assert_eq!(time_bytes(86_399), [23, 59, 59]);
    }

    #[test]
    fn time_writes_three_bytes() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::none());
        op_time(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false).len(), 3);
    }

    #[test]
    fn cfgig_reads_integer_config() {
        let mut vm = test_vm();
        vm.config.insert("TEST_MISC_CFGIG".into(), "0x1F".into());
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), Operand::imm_str(b"test_misc_cfgig"));
        op_cfgig(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 31);

        // Unknown key leaves the register unchanged.
        let (mut a0, mut a1) = (Operand::r(Reg::L(1)), Operand::imm_str(b"no_such_key"));
        op_cfgig(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(1)).unwrap(), 0);
    }

    #[test]
    fn string_to_value_forms() {
        assert_eq!(string_to_value("0x1f"), 31);
        assert_eq!(string_to_value("0y101"), 5);
        assert_eq!(string_to_value("-12"), -12);
        assert_eq!(string_to_value("12.5"), 12);
        assert_eq!(string_to_value("abc"), 0);
        assert_eq!(string_to_value("0xzz"), 0);
        assert_eq!(string_to_value(""), 0);
    }

    #[test]
    fn cfgsg_reads_string_config() {
        let mut vm = test_vm();
        vm.config.insert("TEST_MISC_CFGSG".into(), "abc".into());
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), Operand::imm_str(b"TEST_MISC_CFGSG"));
        op_cfgsg(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false), b"abc");
    }

    #[test]
    fn cfgis_stores_decimal_value() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm_str(b"test_misc_cfgis"), Operand::imm32(42));
        op_cfgis(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.config_property("TEST_MISC_CFGIS"), Some("42"));
    }

    #[test]
    fn shmset_shmget_roundtrip() {
        let mut vm = test_vm();
        vm.regs.string_mut(Reg::S(0)).unwrap().set(b"hello", false);
        let (mut a0, mut a1) = (Operand::imm_str(b"key_misc_shm"), Operand::r(Reg::S(0)));
        op_shmset(&mut vm, oc(), &mut a0, &mut a1).unwrap();

        let (mut a0, mut a1) = (Operand::r(Reg::S(1)), Operand::imm_str(b"KEY_MISC_SHM"));
        op_shmget(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(!vm.flags.carry);
        assert_eq!(vm.regs.string(Reg::S(1)).unwrap().get(false), b"hello");
    }

    #[test]
    fn shmget_unknown_key_sets_carry() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::S(2)), Operand::imm_str(b"key_misc_missing"));
        op_shmget(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.carry);
        assert!(vm.regs.string(Reg::S(2)).unwrap().get(false).is_empty());
    }

    #[test]
    fn iupdate_sets_progress_text() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm_str(b"working"), Operand::none());
        op_iupdate(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.st_misc.info_progress_text, "working");
    }

    #[test]
    fn irange_resets_position() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm32(100), Operand::none());
        op_irange(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.st_misc.info_progress_range, 100);
        assert_eq!(vm.st_misc.info_progress_pos, -1);
    }

    #[test]
    fn iincpos_adds_and_clamps() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm32(100), Operand::none());
        op_irange(&mut vm, oc(), &mut a0, &mut a1).unwrap();

        let (mut a0, mut a1) = (Operand::imm32(30), Operand::none());
        op_iincpos(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.st_misc.info_progress_pos, 30);

        let (mut a0, mut a1) = (Operand::imm32(80), Operand::none());
        op_iincpos(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.st_misc.info_progress_pos, 100);
    }

    /// Content: "line one\r\nline two\nline three" (last line has no break).
    const FILE_CONTENT: &[u8] = b"line one\r\nline two\nline three";

    #[test]
    fn fopen_missing_file_raises() {
        let mut vm = test_vm();
        let missing = std::env::temp_dir().join("best_misc_test_does_not_exist.txt");
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), path_operand(&missing));
        let r = op_fopen(&mut vm, oc(), &mut a0, &mut a1);
        assert!(matches!(
            r,
            Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0006))
        ));
    }

    #[test]
    fn fopen_fread_fclose() {
        let path = temp_file("fopen", FILE_CONTENT);
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), path_operand(&path));
        op_fopen(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let handle = vm.regs.value(Reg::L(0)).unwrap();
        assert_eq!(handle, 0);

        let (mut a0, mut a1) = (Operand::r(Reg::B(0)), Operand::imm8(handle as u8));
        op_fread(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::B(0)).unwrap(), b'l' as u32);
        assert!(!vm.flags.carry);

        let (mut a0, mut a1) = (Operand::imm8(handle as u8), Operand::none());
        op_fclose(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        // Closed handle: fread raises BIP_0006.
        let (mut a0, mut a1) = (Operand::r(Reg::B(1)), Operand::imm8(handle as u8));
        assert!(op_fread(&mut vm, oc(), &mut a0, &mut a1).is_err());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn freadln_reads_lines_and_quirk_on_last_line() {
        let path = temp_file("freadln", FILE_CONTENT);
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), path_operand(&path));
        op_fopen(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let h = Operand::imm8(0);

        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), h.clone());
        op_freadln(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false), b"line one");
        assert!(!vm.flags.carry);

        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), h.clone());
        op_freadln(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.string(Reg::S(0)).unwrap().get(false), b"line two");

        // "line three" has no line break: C# returns null, so empty + carry.
        let (mut a0, mut a1) = (Operand::r(Reg::S(0)), h.clone());
        op_freadln(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.regs.string(Reg::S(0)).unwrap().get(false).is_empty());
        assert!(vm.flags.carry);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn ftell_and_fseek() {
        let path = temp_file("ftell", FILE_CONTENT);
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), path_operand(&path));
        op_fopen(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let h = Operand::imm8(0);

        let (mut a0, mut a1) = (Operand::r(Reg::B(0)), h.clone());
        op_fread(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::L(1)), h.clone());
        op_ftell(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(1)).unwrap(), 1);

        let (mut a0, mut a1) = (h.clone(), Operand::imm32(5));
        op_fseek(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::L(1)), h.clone());
        op_ftell(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(1)).unwrap(), 5);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn fseekln_and_ftellln() {
        let path = temp_file("ftellln", FILE_CONTENT);
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), path_operand(&path));
        op_fopen(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let h = Operand::imm8(0);

        // Seek to line 1 (0-based): "line one\r\n" is 10 bytes.
        let (mut a0, mut a1) = (h.clone(), Operand::imm32(1));
        op_fseekln(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let (mut a0, mut a1) = (Operand::r(Reg::L(1)), h.clone());
        op_ftell(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(1)).unwrap(), 10);

        let (mut a0, mut a1) = (Operand::r(Reg::L(2)), h.clone());
        op_ftellln(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert_eq!(vm.regs.value(Reg::L(2)).unwrap(), 1);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn fread_at_eof_sets_carry() {
        let path = temp_file("fread_eof", b"A");
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::r(Reg::L(0)), path_operand(&path));
        op_fopen(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        let h = Operand::imm8(0);
        let (mut a0, mut a1) = (Operand::r(Reg::B(0)), h.clone());
        op_fread(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(!vm.flags.carry);
        op_fread(&mut vm, oc(), &mut a0, &mut a1).unwrap();
        assert!(vm.flags.carry);
        assert_eq!(vm.regs.value(Reg::B(0)).unwrap(), 0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn fclose_invalid_handle_raises() {
        let mut vm = test_vm();
        let (mut a0, mut a1) = (Operand::imm8(9), Operand::none());
        assert!(matches!(
            op_fclose(&mut vm, oc(), &mut a0, &mut a1),
            Err(VmError::Raised(ErrorCode::EDIABAS_BIP_0006))
        ));
    }
}
