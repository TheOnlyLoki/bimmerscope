//! Opcodes: tabset, tabseek, tabget, tabline, tabseeku, tabsetex, tabcols, tabrows.
//! Ported from `EdOperations.cs` and the table helpers in `EdiabasNet.cs`.
//!
//! Row index convention: C# `TableEntries[0]` is the header row and data row
//! `r` is `TableEntries[r + 1]`. `sgbd::Table.rows` holds only the data rows,
//! so C# row `r` is `t.rows[r]` here. `TableEntries.Length - 1` == `Rows` ==
//! `t.rows.len()`. The header is `t.columns`. C# `GetTableRows` + 1 (the
//! "including header" count of `tabrows`) therefore becomes `t.rows.len() + 1`.

use std::sync::Arc;

use super::OpCode;
use crate::operand::{Operand, Value};
use crate::vm::Vm;
use crate::{ErrorCode, VmResult};

/// Per-group VM state (fields the C# keeps on `EdiabasNet`).
#[derive(Debug)]
pub struct State {
    /// C#: `_tableIndex`. Index into `vm.sgbd.tables`, -1 = no table selected.
    pub table_index: i32,
    /// C#: `_tableRowIndex`. 0-based data row (header excluded), -1 = no row selected.
    pub table_row_index: i32,
    /// C#: `_tableFs` / `_tableInfosExt`: tables from another SGBD (uses file
    /// or `tabsetex` base file). `None` = tables of `vm.sgbd`.
    pub ext: Option<Arc<sgbd::Sgbd>>,
}

impl Default for State {
    fn default() -> Self {
        Self { table_index: -1, table_row_index: -1, ext: None }
    }
}

/// C#: `CloseTableFs`.
fn close_table_fs(vm: &mut Vm) {
    vm.st_table = State::default();
}

/// Tables of the active table file (C#: `GetTableInfos(GetTableFs())`).
fn tables(vm: &Vm) -> &[sgbd::Table] {
    vm.st_table.ext.as_ref().map_or(&vm.sgbd.tables, |s| &s.tables)
}

/// `<EcuPath>/<base>.prg`, else `.grp`, matched case-insensitively.
fn find_base_file(vm: &Vm, base: &str) -> Option<std::path::PathBuf> {
    let dir = vm.config_property("EcuPath")?;
    let entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).collect();
    ["prg", "grp"].iter().find_map(|ext| {
        let want = format!("{base}.{ext}");
        entries.iter().find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&want))).cloned()
    })
}

/// Shared tail of tabset / tabsetex: assign the looked-up table, keep the row
/// only if the table index did not change.
fn assign_table(vm: &mut Vm, idx: i32, found: bool, last_idx: i32, last_row: i32) -> VmResult<()> {
    if !found {
        vm.set_error(ErrorCode::EDIABAS_BIP_0010)?;
    }
    vm.st_table.table_index = idx;
    vm.st_table.table_row_index = if idx == last_idx { last_row } else { -1 };
    Ok(())
}

/// C#: `OpTabset` (tabset)
pub(crate) fn op_tabset(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let (last_idx, last_row) = (vm.st_table.table_index, vm.st_table.table_row_index);
    close_table_fs(vm);
    // C#: `_sgbdBaseFs` is the uses file while an inherited job runs. Its
    // tables are searched first, then the main SGBD.
    let base = (!Arc::ptr_eq(&vm.code, &vm.sgbd)).then(|| vm.code.clone());
    let has_base = base.is_some();
    vm.st_table.ext = base;
    let name = vm.op_string(arg0)?;
    let (mut idx, mut found) = find_table(vm, &name);
    if !found && has_base {
        close_table_fs(vm);
        (idx, found) = find_table(vm, &name);
    }
    assign_table(vm, idx, found, last_idx, last_row)
}

/// C#: `OpTabseek` (tabseek)
pub(crate) fn op_tabseek(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let idx = vm.st_table.table_index;
    if idx < 0 {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0010);
    }
    let column = vm.op_string(arg0)?;
    let value = vm.op_string(arg1)?;
    let (row, found) = seek_string(vm, idx, &column, &value);
    if row < 0 {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0010);
    }
    vm.st_table.table_row_index = row;
    vm.flags.zero = !found;
    Ok(())
}

/// C#: `OpTabget` (tabget)
pub(crate) fn op_tabget(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let idx = vm.st_table.table_index;
    if idx < 0 {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0010);
    }
    let column = vm.op_string(arg1)?;
    let (entry, column_invalid) = get_table_entry(vm, idx, vm.st_table.table_row_index, &column);
    match entry {
        Some(entry) => vm.op_set_string(arg0, &entry),
        None => {
            // EDIABAS returns an empty string, without error, when no row is
            // selected and the column exists.
            if vm.st_table.table_row_index < 0 && !column_invalid {
                return vm.op_set_string(arg0, "");
            }
            vm.set_error(ErrorCode::EDIABAS_BIP_0010)
        }
    }
}

/// C#: `OpTabline` (tabline)
pub(crate) fn op_tabline(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let idx = vm.st_table.table_index;
    if idx < 0 {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0010);
    }
    let line = vm.op_value(arg0, 0)?;
    let (row, found) = get_table_line(vm, idx, line);
    // C# does not return here: the row and the zero flag are still updated.
    if row < 0 {
        vm.set_error(ErrorCode::EDIABAS_BIP_0010)?;
    }
    vm.st_table.table_row_index = row;
    vm.flags.zero = !found;
    Ok(())
}

/// C#: `OpTabrows` (tabrows)
pub(crate) fn op_tabrows(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let idx = vm.st_table.table_index;
    let rows = if idx < 0 {
        0
    } else {
        // C# GetTableRows returns 0 for an invalid index; +1 counts the header.
        let data_rows = table(vm, idx).map_or(0, |t| t.rows.len() as u32);
        data_rows.wrapping_add(1)
    };
    vm.op_set_raw(arg0, Value::Int(rows), 1)
}

/// C#: `OpTabseeku` (tabseeku)
pub(crate) fn op_tabseeku(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let idx = vm.st_table.table_index;
    if idx < 0 {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0010);
    }
    let column = vm.op_string(arg0)?;
    let value = vm.op_value(arg1, 0)?;
    let (row, found) = seek_value(vm, idx, &column, value);
    if row < 0 {
        return vm.set_error(ErrorCode::EDIABAS_BIP_0010);
    }
    vm.st_table.table_row_index = row;
    vm.flags.zero = !found;
    Ok(())
}

/// C#: `OpTabsetex` (tabsetex)
pub(crate) fn op_tabsetex(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, arg1: &mut Operand) -> VmResult<()> {
    let (last_idx, last_row) = (vm.st_table.table_index, vm.st_table.table_row_index);
    let base = vm.op_string(arg1)?;
    if !base.is_empty() {
        let Some(path) = find_base_file(vm, &base) else {
            return vm.set_error(ErrorCode::EDIABAS_SYS_0002);
        };
        close_table_fs(vm);
        match sgbd::Sgbd::open(&path) {
            Ok(s) => vm.st_table.ext = Some(Arc::new(s)),
            Err(_) => return vm.set_error(ErrorCode::EDIABAS_SYS_0002),
        }
    }
    let name = vm.op_string(arg0)?;
    let (idx, found) = find_table(vm, &name);
    assign_table(vm, idx, found, last_idx, last_row)
}

/// C#: `OpTabcols` (tabcols)
pub(crate) fn op_tabcols(vm: &mut Vm, _oc: &OpCode, arg0: &mut Operand, _arg1: &mut Operand) -> VmResult<()> {
    let idx = vm.st_table.table_index;
    let columns = if idx < 0 { 0 } else { table(vm, idx).map_or(0, |t| t.columns.len() as u32) };
    vm.op_set_raw(arg0, Value::Int(columns), 1)
}

/// C#: `EdiabasNet.StringToValue(number, out valid)`. Returns `(value, valid)`.
/// Hex `0x..`, binary `0y..`, otherwise decimal (text after `.` or `,` is
/// ignored). Anything unparsable gives `(0, false)`, as C# does. Note that
/// `valid` is not checked by the caller, so an invalid cell still acts as 0.
fn string_to_value(number: &str) -> (i64, bool) {
    let s = number.trim_end();
    if s.is_empty() {
        return (0, false);
    }
    let lower = s.to_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        // The first digit must be a hex digit; the rest is parsed by the radix
        // conversion (any other character makes it invalid).
        let first_ok = hex.chars().next().is_some_and(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if hex.is_empty() || !first_ok {
            return (0, false);
        }
        return match i64::from_str_radix(&s[2..], 16) {
            Ok(v) => (v, true),
            Err(_) => (0, false),
        };
    }
    if lower.starts_with("0y") {
        return match i64::from_str_radix(&s[2..], 2) {
            Ok(v) => (v, true),
            Err(_) => (0, false),
        };
    }
    if lower == "-" || lower == "--" || lower.chars().next().is_some_and(char::is_alphabetic) {
        return (0, false);
    }
    let t = s.trim_start();
    let t = match t.find(['.', ',']) {
        Some(i) => &t[..i],
        None => t,
    };
    match t.trim().parse::<i64>() {
        Ok(v) => (v, true),
        Err(_) => (0, false),
    }
}

/// C# `ToUpper(Culture)`: per-character simple uppercase, length preserved.
fn c_upper(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut u = c.to_uppercase();
            match (u.next(), u.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        })
        .collect()
}

/// The table at `idx`, if valid (C#: `tableIdx < 0 || tableIdx >= Length` gives null).
fn table(vm: &Vm, idx: i32) -> Option<&sgbd::Table> {
    usize::try_from(idx).ok().and_then(|i| tables(vm).get(i))
}

/// Cell of a data row; a short row reads as "" (C# rows are always full width).
fn cell(row: &[String], col: usize) -> &str {
    row.get(col).map_or("", String::as_str)
}

/// C#: `GetTableColumnIdx`. First match wins. C# would throw on a duplicate
/// column name (`Dictionary.Add`); that is not reproduced.
fn column_index(t: &sgbd::Table, name: &str) -> Option<usize> {
    let want = c_upper(name);
    t.columns.iter().position(|c| c_upper(c) == want)
}

/// C#: `GetTableIndex(fs, name, out found)`. Not found gives the last table
/// index (`Length - 1`), as C# does, with `found = false`.
fn find_table(vm: &Vm, name: &str) -> (i32, bool) {
    let want = c_upper(name);
    let tables = tables(vm);
    match tables.iter().position(|t| c_upper(&t.name) == want) {
        Some(i) => (i as i32, true),
        None => (tables.len() as i32 - 1, false),
    }
}

/// C#: `GetTableLine(fs, tableIdx, line, out found)`.
fn get_table_line(vm: &Vm, idx: i32, line: u32) -> (i32, bool) {
    let Some(t) = table(vm, idx) else { return (-1, false) };
    let rows = t.rows.len();
    if line as usize >= rows {
        // Select the last line. For an empty table this is -1, as in C#.
        ((rows as i32) - 1, false)
    } else {
        (line as i32, true)
    }
}

/// C#: `GetTableEntry(fs, tableIdx, rowIdx, columnName, out columnInvalid)`.
/// Returns `(entry, column_invalid)`.
fn get_table_entry(vm: &Vm, idx: i32, row: i32, column: &str) -> (Option<String>, bool) {
    let Some(t) = table(vm, idx) else { return (None, false) };
    let Some(col) = column_index(t, column) else { return (None, true) };
    if row < 0 || row as usize >= t.rows.len() {
        return (None, false);
    }
    (Some(cell(&t.rows[row as usize], col).to_string()), false)
}

/// Shared body of the two `SeekTable` overloads. The C# dictionary keeps the
/// first row for each key, so a linear scan for the first match gives the same
/// result. A miss selects the last row (`Rows - 1`) with `found = false`.
fn seek_rows<K: PartialEq>(t: &sgbd::Table, col: usize, want: &K, key: impl Fn(&str) -> K) -> (i32, bool) {
    match t.rows.iter().position(|r| key(cell(r, col)) == *want) {
        Some(i) => (i as i32, true),
        None => (t.rows.len() as i32 - 1, false),
    }
}

/// C#: `SeekTable(fs, tableIdx, columnName, string valueName, out found)`.
/// Compares upper-cased strings.
fn seek_string(vm: &Vm, idx: i32, column: &str, value: &str) -> (i32, bool) {
    let Some(t) = table(vm, idx) else { return (-1, false) };
    let Some(col) = column_index(t, column) else { return (-1, false) };
    seek_rows(t, col, &c_upper(value), c_upper)
}

/// C#: `SeekTable(fs, tableIdx, columnName, EdValueType value, out found)`.
/// Cells are parsed with `StringToValue`. Invalid cells become 0, as in C#.
fn seek_value(vm: &Vm, idx: i32, column: &str, value: u32) -> (i32, bool) {
    let Some(t) = table(vm, idx) else { return (-1, false) };
    let Some(col) = column_index(t, column) else { return (-1, false) };
    seek_rows(t, col, &value, |s| string_to_value(s).0 as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operand::Operand;
    use crate::regs::Reg;
    use crate::ops::OpFn;
    use std::sync::Arc;

    fn table_t() -> sgbd::Table {
        sgbd::Table {
            name: "T".into(),
            columns: vec!["NR".into(), "TEXT".into()],
            rows: vec![vec!["0x01".into(), "eins".into()], vec!["0xXY".into(), "default".into()]],
        }
    }

    fn table_u() -> sgbd::Table {
        sgbd::Table { name: "U".into(), columns: vec!["A".into()], rows: vec![vec!["1".into()]] }
    }

    /// One SGBD with the test tables T (2 data rows) and U (1 data row).
    fn vm_with(tables: Vec<sgbd::Table>) -> Vm {
        let mut s = sgbd::Sgbd::empty();
        s.tables = tables;
        Vm::new(Arc::new(s))
    }

    fn vm() -> Vm {
        vm_with(vec![table_t(), table_u()])
    }

    fn run(f: OpFn, vm: &mut Vm, a0: &mut Operand, a1: &mut Operand) -> VmResult<()> {
        f(vm, &super::super::OPCODES[0], a0, a1)
    }

    /// Stores `text` (NUL terminated) in string register `reg`.
    fn set_s(vm: &mut Vm, reg: Reg, text: &str) {
        vm.reg_set_array(reg, format!("{text}\0").as_bytes(), false).unwrap();
    }

    fn get_s(vm: &mut Vm, reg: Reg) -> String {
        vm.op_string(&Operand::r(reg)).unwrap()
    }

    fn trap(vm: &mut Vm, code: ErrorCode) {
        vm.error_trap_mask = 1 << code.trap_bit();
    }

    fn tabset(vm: &mut Vm, name: &str) -> VmResult<()> {
        set_s(vm, Reg::S(0), name);
        run(op_tabset, vm, &mut Operand::r(Reg::S(0)), &mut Operand::none())
    }

    #[test]
    fn string_to_value_cases() {
        assert_eq!(string_to_value("0x01"), (1, true));
        assert_eq!(string_to_value("0xXY"), (0, false));
        assert_eq!(string_to_value("0x"), (0, false));
        assert_eq!(string_to_value("0y101"), (5, true));
        assert_eq!(string_to_value("12"), (12, true));
        assert_eq!(string_to_value("12,5"), (12, true));
        assert_eq!(string_to_value("-3"), (-3, true));
        assert_eq!(string_to_value("eins"), (0, false));
        assert_eq!(string_to_value(""), (0, false));
    }

    #[test]
    fn tabset_selects_table_case_insensitive() {
        let mut vm = vm();
        tabset(&mut vm, "t").unwrap();
        assert_eq!(vm.st_table.table_index, 0);
        assert_eq!(vm.st_table.table_row_index, -1);
        tabset(&mut vm, "U").unwrap();
        assert_eq!(vm.st_table.table_index, 1);
    }

    #[test]
    fn tabset_unknown_raises_and_leaves_reset_state() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        // C# resets the selection before the lookup, so an untrapped error
        // leaves no table selected.
        let r = tabset(&mut vm, "NOPE");
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
        assert_eq!(vm.st_table.table_index, -1);
        assert_eq!(vm.st_table.table_row_index, -1);
    }

    #[test]
    fn tabset_unknown_trapped_selects_last_table() {
        let mut vm = vm();
        trap(&mut vm, ErrorCode::EDIABAS_BIP_0010);
        tabset(&mut vm, "NOPE").unwrap();
        // Not found gives the last table index, as C# GetTableIndex does.
        assert_eq!(vm.st_table.table_index, 1);
        assert_eq!(vm.st_table.table_row_index, -1);
    }

    #[test]
    fn tabset_same_table_keeps_row() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        vm.st_table.table_row_index = 1;
        tabset(&mut vm, "T").unwrap();
        assert_eq!(vm.st_table.table_row_index, 1);
        tabset(&mut vm, "U").unwrap();
        assert_eq!(vm.st_table.table_row_index, -1);
    }

    #[test]
    fn tabseek_found_sets_row_and_clears_zero() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(0), "nr");
        set_s(&mut vm, Reg::S(1), "0x01");
        vm.flags.zero = true;
        run(op_tabseek, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1))).unwrap();
        assert_eq!(vm.st_table.table_row_index, 0);
        assert!(!vm.flags.zero);
    }

    #[test]
    fn tabseek_miss_selects_last_row_and_sets_zero() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(0), "NR");
        set_s(&mut vm, Reg::S(1), "0x7F");
        run(op_tabseek, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1))).unwrap();
        assert_eq!(vm.st_table.table_row_index, 1);
        assert!(vm.flags.zero);
    }

    #[test]
    fn tabseek_errors() {
        let mut vm = vm();
        // No table selected.
        set_s(&mut vm, Reg::S(0), "NR");
        set_s(&mut vm, Reg::S(1), "0x01");
        let r = run(op_tabseek, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
        // Unknown column: -1, error, row unchanged.
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(0), "XX");
        let r = run(op_tabseek, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
        assert_eq!(vm.st_table.table_row_index, -1);
    }

    #[test]
    fn tabseek_empty_table_raises() {
        let mut vm = vm_with(vec![sgbd::Table { name: "E".into(), columns: vec!["NR".into()], rows: vec![] }]);
        tabset(&mut vm, "E").unwrap();
        set_s(&mut vm, Reg::S(0), "NR");
        set_s(&mut vm, Reg::S(1), "1");
        // Miss on an empty table gives row -1, which is an error in C#.
        let r = run(op_tabseek, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
    }

    #[test]
    fn tabseeku_found_and_miss() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(0), "NR");
        // Row 0 is "0x01" = 1.
        run(op_tabseeku, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::imm32(1)).unwrap();
        assert_eq!(vm.st_table.table_row_index, 0);
        assert!(!vm.flags.zero);
        // Quirk: the invalid "0xXY" parses as 0, so value 0 matches row 1.
        run(op_tabseeku, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::imm32(0)).unwrap();
        assert_eq!(vm.st_table.table_row_index, 1);
        assert!(!vm.flags.zero);
        // Miss selects the last row and sets zero.
        run(op_tabseeku, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::imm32(5)).unwrap();
        assert_eq!(vm.st_table.table_row_index, 1);
        assert!(vm.flags.zero);
    }

    #[test]
    fn tabget_returns_entry_of_selected_row() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(0), "NR");
        set_s(&mut vm, Reg::S(1), "0x01");
        run(op_tabseek, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1))).unwrap();
        set_s(&mut vm, Reg::S(2), "TEXT");
        run(op_tabget, &mut vm, &mut Operand::r(Reg::S(3)), &mut Operand::r(Reg::S(2))).unwrap();
        assert_eq!(get_s(&mut vm, Reg::S(3)), "eins");
        // Column lookup is case-insensitive.
        set_s(&mut vm, Reg::S(2), "text");
        run(op_tabget, &mut vm, &mut Operand::r(Reg::S(3)), &mut Operand::r(Reg::S(2))).unwrap();
        assert_eq!(get_s(&mut vm, Reg::S(3)), "eins");
    }

    #[test]
    fn tabget_no_row_gives_empty_string() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(2), "TEXT");
        set_s(&mut vm, Reg::S(3), "old");
        run(op_tabget, &mut vm, &mut Operand::r(Reg::S(3)), &mut Operand::r(Reg::S(2))).unwrap();
        assert_eq!(get_s(&mut vm, Reg::S(3)), "");
    }

    #[test]
    fn tabget_errors() {
        let mut vm = vm();
        set_s(&mut vm, Reg::S(2), "TEXT");
        // No table selected.
        let r = run(op_tabget, &mut vm, &mut Operand::r(Reg::S(3)), &mut Operand::r(Reg::S(2)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
        // Unknown column with no row selected is still an error (column invalid).
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(2), "XX");
        let r = run(op_tabget, &mut vm, &mut Operand::r(Reg::S(3)), &mut Operand::r(Reg::S(2)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
        // Unknown column with a row selected is an error too.
        vm.st_table.table_row_index = 0;
        let r = run(op_tabget, &mut vm, &mut Operand::r(Reg::S(3)), &mut Operand::r(Reg::S(2)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
    }

    #[test]
    fn tabline_selects_row_by_number() {
        let mut vm = vm();
        tabset(&mut vm, "T").unwrap();
        run(op_tabline, &mut vm, &mut Operand::imm32(1), &mut Operand::none()).unwrap();
        assert_eq!(vm.st_table.table_row_index, 1);
        assert!(!vm.flags.zero);
        // Line beyond the table selects the last row and sets zero.
        run(op_tabline, &mut vm, &mut Operand::imm32(7), &mut Operand::none()).unwrap();
        assert_eq!(vm.st_table.table_row_index, 1);
        assert!(vm.flags.zero);
    }

    #[test]
    fn tabline_without_table_raises() {
        let mut vm = vm();
        let r = run(op_tabline, &mut vm, &mut Operand::imm32(0), &mut Operand::none());
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
    }

    #[test]
    fn tabcols_and_tabrows() {
        let mut vm = vm();
        // No table: both return 0.
        run(op_tabcols, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);
        run(op_tabrows, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 0);

        tabset(&mut vm, "T").unwrap();
        run(op_tabcols, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 2);
        // 2 data rows + header.
        run(op_tabrows, &mut vm, &mut Operand::r(Reg::L(0)), &mut Operand::none()).unwrap();
        assert_eq!(vm.regs.value(Reg::L(0)).unwrap(), 3);
    }

    #[test]
    fn tabsetex_without_base_behaves_like_tabset() {
        let mut vm = vm();
        set_s(&mut vm, Reg::S(0), "U");
        set_s(&mut vm, Reg::S(1), "");
        run(op_tabsetex, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1))).unwrap();
        assert_eq!(vm.st_table.table_index, 1);
        // Not found: error, and the selection is not reset (no file switch).
        tabset(&mut vm, "T").unwrap();
        set_s(&mut vm, Reg::S(0), "NOPE");
        let r = run(op_tabsetex, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_BIP_0010))));
        assert_eq!(vm.st_table.table_index, 0);
    }

    #[test]
    fn tabsetex_with_missing_base_file_raises_sys_0002() {
        let mut vm = vm();
        vm.config.insert("EcuPath".into(), std::env::temp_dir().to_string_lossy().into_owned());
        set_s(&mut vm, Reg::S(0), "U");
        set_s(&mut vm, Reg::S(1), "no_such_bmwa_base_file");
        let r = run(op_tabsetex, &mut vm, &mut Operand::r(Reg::S(0)), &mut Operand::r(Reg::S(1)));
        assert!(matches!(r, Err(crate::VmError::Raised(ErrorCode::EDIABAS_SYS_0002))));
    }
}
