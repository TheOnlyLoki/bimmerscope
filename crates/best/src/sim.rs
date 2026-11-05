//! Simulation interface: answers request telegrams from an EdiabasLib
//! `.sim` file, so jobs can run without a car.
//!
//! Ported from EdiabasLib (`EdSimFile.cs`, `IniFile.cs`, the simulation
//! parts of `EdInterfaceBase.cs` and `EdInterfaceObd.cs`, GPL-3.0).
//! Only the OBD interface in simulation mode is ported (no EDIC/ISO-TP).
//!
//! Sim file format (INI):
//! - `[POWERSUPPLY] UBatt=<mV>`, `[IGNITION] Ignition=<mV>`.
//! - `[<prefix>REQUEST]` / `[<prefix>RESPONSE]`: same key in both sections
//!   pairs a request with a response. Optional prefix `<hex>.` binds the
//!   entries to an ECU/wake address. Identical requests under several keys
//!   (e.g. `X_0`, `X_1`) collect their responses, which are returned in turn.
//! - `[<prefix>KEYBYTES]`: key bytes, also returned in turn.
//! - Byte syntax: `12` hex byte, `X2`/`1X` nibble wildcard (request only),
//!   `..` variable length tail (request only), `12+34` operator with a
//!   constant, `12+[3]` operator with request byte 3, `12+#0` operator with
//!   ECU address byte 0/1, `00^$0` / `00+$0` XOR/sum checksum over the
//!   preceding bytes (`$n`, n > 0: only the first n bytes).

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use crate::ErrorCode;
use crate::iface::{IfResult, Interface};
use crate::telegram::{TRANS_BUFFER_SIZE, calc_checksum_bmw_fast, tel_length_bmw_fast};

pub const DEFAULT_BATTERY_VOLT: i32 = 12000;
pub const DEFAULT_IGNITION_VOLT: i32 = 12000;
const SECTION_POWER_SUPPLY: &str = "POWERSUPPLY";
const SECTION_IGNITION: &str = "IGNITION";
const SECTION_REQUEST: &str = "REQUEST";
const SECTION_RESPONSE: &str = "RESPONSE";
const SECTION_KEYBYTES: &str = "KEYBYTES";

// ---------------------------------------------------------------------------
// IniFile (C#: IniFile, read-only subset)
// ---------------------------------------------------------------------------

struct IniSection {
    name: String,
    /// Keys in file order (C# Dictionary keeps insertion order).
    keys: Vec<(String, String)>,
    /// Lower-case key to index in `keys` (C#: InvariantCultureIgnoreCase).
    index: HashMap<String, usize>,
}

struct IniFile {
    sections: Vec<IniSection>,
    index: HashMap<String, usize>,
}

impl IniFile {
    /// C#: `IniFile.Refresh`. Only the first occurrence of a section or key
    /// is loaded. Lines without `=` (comments) are ignored.
    fn parse(text: &str) -> IniFile {
        let mut ini = IniFile {
            sections: Vec::new(),
            index: HashMap::new(),
        };
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let s = line.trim();
            if let Some(name) = parse_section_name(s) {
                let lower = name.to_lowercase();
                if ini.index.contains_key(&lower) {
                    current = None;
                } else {
                    ini.index.insert(lower, ini.sections.len());
                    current = Some(ini.sections.len());
                    ini.sections.push(IniSection {
                        name: name.to_string(),
                        keys: Vec::new(),
                        index: HashMap::new(),
                    });
                }
            } else if let (Some(idx), Some((key, value))) = (current, parse_key_value_pair(s)) {
                let section = &mut ini.sections[idx];
                let lower = key.to_lowercase();
                if !section.index.contains_key(&lower) {
                    section.index.insert(lower, section.keys.len());
                    section.keys.push((key.to_string(), value.to_string()));
                }
            }
        }
        ini
    }

    fn section(&self, name: &str) -> Option<&IniSection> {
        self.index
            .get(&name.to_lowercase())
            .map(|&i| &self.sections[i])
    }

    fn get_value(&self, section: &str, key: &str) -> Option<&str> {
        let section = self.section(section)?;
        section
            .index
            .get(&key.to_lowercase())
            .map(|&i| section.keys[i].1.as_str())
    }

    /// C#: `GetValue(section, key, int default)`; unparsable gives the default.
    fn get_int(&self, section: &str, key: &str, default: i32) -> i32 {
        match self.get_value(section, key) {
            Some(v) => parse_int_any(v).unwrap_or(default),
            None => default,
        }
    }
}

fn parse_section_name(line: &str) -> Option<&str> {
    if !line.starts_with('[') || !line.ends_with(']') || line.len() < 3 {
        return None;
    }
    Some(&line[1..line.len() - 1])
}

fn parse_key_value_pair(line: &str) -> Option<(&str, &str)> {
    let i = line.find('=')?;
    if i == 0 {
        return None;
    }
    let key = line[..i].trim();
    if key.is_empty() {
        return None;
    }
    Some((key, line[i + 1..].trim()))
}

/// C#: `int.Parse(s, NumberStyles.Any, InvariantCulture)`, approximated:
/// integer, or a float with no fractional part.
fn parse_int_any(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Ok(v) = s.parse::<i32>() {
        return Some(v);
    }
    let f = s.parse::<f64>().ok()?;
    if f.is_finite() && f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
        return Some(f as i32);
    }
    None
}

/// C#: `TryParse(s, NumberStyles.HexNumber)`: hex digits only, leading and
/// trailing white space allowed, no sign, no `0x`.
fn parse_hex(s: &str, max: u64) -> Option<u64> {
    let s = s.trim_matches(|c: char| c == ' ' || ('\u{9}'..='\u{d}').contains(&c));
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digits = s.trim_start_matches('0');
    if digits.len() > 16 {
        return None;
    }
    let v = if digits.is_empty() {
        0
    } else {
        u64::from_str_radix(digits, 16).ok()?
    };
    if v > max {
        return None;
    }
    Some(v)
}

fn parse_hex_u8(s: &str) -> Option<u8> {
    parse_hex(s, 0xFF).map(|v| v as u8)
}

fn parse_hex_u32(s: &str) -> Option<u32> {
    parse_hex(s, 0xFFFF_FFFF).map(|v| v as u32)
}

// ---------------------------------------------------------------------------
// EdSimFile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperatorType {
    And,
    Or,
    Xor,
    Plus,
    Minus,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperatorDataType {
    RequestIdx,
    EcuAddr,
    Checksum,
}

#[derive(Debug, Clone)]
struct DataItem {
    data_value: u8,
    data_mask: Option<u8>,
    operator: Option<OperatorType>,
    operator_data: Option<OperatorDataType>,
    operator_value: Option<u8>,
    operator_index: Option<u32>,
    var_length: bool,
}

impl DataItem {
    /// C#: `DataItem.Equals`. Quirk: `OperatorData` and `VarLength` are not
    /// compared, so `12+[1]` equals `12+#1`.
    fn same(&self, other: &DataItem) -> bool {
        self.data_value == other.data_value
            && self.data_mask == other.data_mask
            && self.operator == other.operator
            && self.operator_value == other.operator_value
            && self.operator_index == other.operator_index
    }
}

#[derive(Debug, Clone)]
struct ResponseInfo {
    request_data: Vec<DataItem>,
    response_data_list: Vec<Vec<DataItem>>,
    ecu_addr: Option<i32>,
    response_index: usize,
}

impl ResponseInfo {
    fn new(
        request_data: Vec<DataItem>,
        response_data: Vec<DataItem>,
        ecu_addr: Option<i32>,
    ) -> Self {
        ResponseInfo {
            request_data,
            response_data_list: vec![response_data],
            ecu_addr,
            response_index: 0,
        }
    }

    /// Returns the next response and advances the cycling index.
    fn next_response(&mut self) -> Option<Vec<DataItem>> {
        let mut idx = self.response_index;
        if idx < self.response_data_list.len() {
            let r = self.response_data_list[idx].clone();
            idx += 1;
            if idx >= self.response_data_list.len() {
                idx = 0;
            }
            self.response_index = idx;
            return Some(r);
        }
        None
    }
}

/// Parsed sim file (C#: `EdSimFile`).
#[derive(Debug, Clone)]
pub struct SimFile {
    file_valid: bool,
    pub ubat_volt: i32,
    pub ubat_current: i32,
    pub ubat_history: i32,
    pub ignition_volt: i32,
    pub ignition_current: i32,
    pub ignition_history: i32,
    response_infos: Vec<ResponseInfo>,
    key_bytes_infos: Vec<ResponseInfo>,
}

impl SimFile {
    /// Reads and parses a sim file. A parse error is no I/O error: check
    /// [`SimFile::file_valid`].
    pub fn load(path: impl AsRef<Path>) -> std::io::Result<SimFile> {
        let bytes = std::fs::read(path)?;
        // C# StreamReader: UTF-8, BOM stripped.
        let text = String::from_utf8_lossy(&bytes);
        Ok(SimFile::parse(
            text.strip_prefix('\u{FEFF}').unwrap_or(&text),
        ))
    }

    /// Parses sim file text (C#: constructor + `ParseIniFile`).
    pub fn parse(text: &str) -> SimFile {
        let ini = IniFile::parse(text);
        let mut sim = SimFile {
            file_valid: false,
            ubat_volt: ini.get_int(SECTION_POWER_SUPPLY, "UBatt", DEFAULT_BATTERY_VOLT),
            ubat_current: ini.get_int(SECTION_POWER_SUPPLY, "UBATTCURRENT", -1),
            ubat_history: ini.get_int(SECTION_POWER_SUPPLY, "UBATTHISTORY", -1),
            ignition_volt: ini.get_int(SECTION_IGNITION, "Ignition", DEFAULT_IGNITION_VOLT),
            ignition_current: ini.get_int(SECTION_IGNITION, "IGNITIONCURRENT", -1),
            ignition_history: ini.get_int(SECTION_IGNITION, "IGNITIONHISTORY", -1),
            response_infos: Vec::new(),
            key_bytes_infos: Vec::new(),
        };
        sim.file_valid = sim.parse_ini(&ini);
        sim.reset_responses();
        sim
    }

    pub fn file_valid(&self) -> bool {
        self.file_valid
    }

    /// C#: `ResetResponses`. Only the request entries; key bytes keep their index.
    pub fn reset_responses(&mut self) {
        for info in &mut self.response_infos {
            info.response_index = 0;
        }
    }

    /// Resets request and key byte indices, like re-reading the file.
    fn reset_all(&mut self) {
        self.reset_responses();
        for info in &mut self.key_bytes_infos {
            info.response_index = 0;
        }
    }

    /// C#: `GetResponse`. Pass 0 tries exact entries only; pass 1 also
    /// entries with wildcards, operators or variable length.
    pub fn get_response(
        &mut self,
        request: &[u8],
        sim_ecu_addr: Option<i32>,
        sim_wake_addr: Option<i32>,
    ) -> Option<Vec<u8>> {
        let ecu_addr = sim_wake_addr.or(sim_ecu_addr);
        for iteration in 0..2 {
            for info in &mut self.response_infos {
                if info.ecu_addr.unwrap_or(-1) != ecu_addr.unwrap_or(-1) {
                    continue;
                }
                if request.len() < info.request_data.len() {
                    continue;
                }
                let var_length = !info.request_data.is_empty() && info.request_data[0].var_length;
                if request.len() != info.request_data.len() && !var_length {
                    continue;
                }
                if iteration == 0 {
                    let variable_data = var_length
                        || info
                            .request_data
                            .iter()
                            .any(|d| d.data_mask.is_some() || d.operator.is_some());
                    if variable_data {
                        continue;
                    }
                }

                let Some(request_info_list) = convert_data(
                    &info.request_data,
                    None,
                    Some(request),
                    sim_ecu_addr,
                    sim_wake_addr,
                ) else {
                    continue;
                };
                let Some(request_data_list) = convert_data(
                    &info.request_data,
                    Some(request),
                    Some(request),
                    sim_ecu_addr,
                    sim_wake_addr,
                ) else {
                    continue;
                };
                if request_info_list != request_data_list {
                    continue;
                }

                let response = info.next_response()?;
                return convert_data(&response, None, Some(request), sim_ecu_addr, sim_wake_addr);
            }
        }
        None
    }

    /// C#: `GetKeyBytes`. Returns the next key bytes of the first entry with
    /// a matching address.
    pub fn get_key_bytes(
        &mut self,
        sim_ecu_addr: Option<i32>,
        sim_wake_addr: Option<i32>,
    ) -> Option<Vec<u8>> {
        let ecu_addr = sim_wake_addr.or(sim_ecu_addr);
        let info = self
            .key_bytes_infos
            .iter_mut()
            .find(|i| i.ecu_addr.unwrap_or(-1) == ecu_addr.unwrap_or(-1))?;
        let response = info.next_response()?;
        convert_data(&response, None, None, sim_ecu_addr, sim_wake_addr)
    }

    /// C#: `ParseIniFile`. On error the entries parsed so far are kept.
    fn parse_ini(&mut self, ini: &IniFile) -> bool {
        for section in &ini.sections {
            let Some(prefix) = strip_suffix_ci(&section.name, SECTION_REQUEST) else {
                continue;
            };
            let ecu_addr = if prefix.is_empty() {
                None
            } else {
                parse_section_ecu_addr(prefix)
            };
            let response_section = format!("{prefix}{SECTION_RESPONSE}");
            for (key, value) in &section.keys {
                let request_value = value.trim();
                if request_value.is_empty() {
                    return false;
                }
                let response_value = ini.get_value(&response_section, key).unwrap_or("").trim();
                if response_value.is_empty() {
                    return false;
                }
                let Some(request_bytes) = parse_hex_string(request_value, true) else {
                    return false;
                };
                let Some(response_bytes) = parse_hex_string(response_value, false) else {
                    return false;
                };

                // C# quirk: the item loop does not break on the first difference.
                let matched = self.response_infos.iter_mut().find(|info| {
                    info.ecu_addr.unwrap_or(-1) == ecu_addr.unwrap_or(-1)
                        && info.request_data.len() == request_bytes.len()
                        && request_bytes
                            .iter()
                            .zip(&info.request_data)
                            .all(|(a, b)| a.same(b))
                });
                match matched {
                    Some(info) => info.response_data_list.push(response_bytes),
                    None => self.response_infos.push(ResponseInfo::new(
                        request_bytes,
                        response_bytes,
                        ecu_addr,
                    )),
                }
            }
        }

        for section in &ini.sections {
            let Some(prefix) = strip_suffix_ci(&section.name, SECTION_KEYBYTES) else {
                continue;
            };
            let ecu_addr = if prefix.is_empty() {
                None
            } else {
                parse_section_ecu_addr(prefix)
            };
            for (_, value) in &section.keys {
                let key_bytes_value = value.trim();
                if key_bytes_value.is_empty() {
                    continue;
                }
                let Some(key_bytes) = parse_hex_string(key_bytes_value, true) else {
                    return false;
                };
                let matched = self
                    .key_bytes_infos
                    .iter_mut()
                    .find(|i| i.ecu_addr.unwrap_or(-1) == ecu_addr.unwrap_or(-1));
                match matched {
                    Some(info) => info.response_data_list.push(key_bytes),
                    None => self.key_bytes_infos.push(ResponseInfo::new(
                        Vec::new(),
                        key_bytes,
                        ecu_addr,
                    )),
                }
            }
        }
        true
    }
}

/// `name` without `suffix` (case-insensitive), or `None`.
fn strip_suffix_ci<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    if name.len() < suffix.len() || !name.is_char_boundary(name.len() - suffix.len()) {
        return None;
    }
    let (head, tail) = name.split_at(name.len() - suffix.len());
    tail.eq_ignore_ascii_case(suffix).then_some(head)
}

/// C#: `ParseSectionEcuAddr`. `12.` gives 0x12. Quirk: a prefix that does
/// not parse gives `None`, so its entries count as address-less.
fn parse_section_ecu_addr(prefix: &str) -> Option<i32> {
    if prefix.chars().count() > 1 && prefix.ends_with('.') {
        // C# int.TryParse(HexNumber) accepts 8 digits as two's complement.
        return parse_hex_u32(&prefix[..prefix.len() - 1]).map(|v| v as i32);
    }
    None
}

/// C#: `ParseHexString`. Comma separated items; `_` or blank is an empty list.
fn parse_hex_string(text: &str, request: bool) -> Option<Vec<DataItem>> {
    if text.is_empty() {
        return None;
    }
    let text_trim = text.trim();
    if text_trim.is_empty() || text_trim == "_" {
        return Some(Vec::new());
    }
    let parts: Vec<&str> = text_trim.split(',').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut var_length = false;
    let mut result = Vec::new();
    for part in parts {
        let part_upper = part.to_uppercase();
        let pt: Vec<char> = part_upper.trim().chars().collect();
        if pt.len() < 2 {
            return None;
        }

        let mut value = String::new();
        let mut mask: u8 = 0xFF;
        if request {
            if pt[0] == '.' && pt[1] == '.' {
                var_length = true;
                break;
            }
            if pt[0] == 'X' {
                mask &= 0x0F;
                value.push('0');
            } else {
                value.push(pt[0]);
            }
            if pt[1] == 'X' {
                mask &= 0xF0;
                value.push('0');
            } else {
                value.push(pt[1]);
            }
        } else {
            value.push(pt[0]);
            value.push(pt[1]);
        }

        let data_value = parse_hex_u8(&value)?;
        let data_mask = (mask != 0xFF).then_some(mask);

        let mut operator = None;
        let mut operator_data = None;
        let mut operator_value = None;
        let mut operator_index = None;
        if pt.len() >= 4 {
            operator = Some(match pt[2] {
                '|' => OperatorType::Or,
                '&' => OperatorType::And,
                '^' => OperatorType::Xor,
                '+' => OperatorType::Plus,
                '-' => OperatorType::Minus,
                '*' => OperatorType::Multiply,
                '/' => OperatorType::Divide,
                _ => return None,
            });
            let op: String = pt[3..].iter().collect();
            if op.starts_with('[') && op.ends_with(']') {
                operator_index = Some(parse_hex_u32(&op[1..op.len() - 1])?);
                operator_data = Some(OperatorDataType::RequestIdx);
            } else if let Some(rest) = op.strip_prefix('#') {
                operator_index = Some(parse_hex_u32(rest)?);
                operator_data = Some(OperatorDataType::EcuAddr);
            } else if let Some(rest) = op.strip_prefix('$') {
                operator_index = Some(parse_hex_u32(rest)?);
                operator_data = Some(OperatorDataType::Checksum);
            } else {
                operator_value = Some(parse_hex_u8(&op)?);
            }
        }

        result.push(DataItem {
            data_value,
            data_mask,
            operator,
            operator_data,
            operator_value,
            operator_index,
            var_length: false,
        });
    }

    if var_length {
        if result.is_empty() {
            return None;
        }
        result[0].var_length = true;
    }
    Some(result)
}

/// C#: `ConvertData`. Builds bytes from `items`; with `input`, the item
/// value is replaced by the input byte at the same position (then masked).
fn convert_data(
    items: &[DataItem],
    input: Option<&[u8]>,
    request: Option<&[u8]>,
    sim_ecu_addr: Option<i32>,
    _sim_wake_addr: Option<i32>,
) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let mut data_value = item.data_value;
        if let Some(&b) = input.and_then(|i| i.get(index)) {
            data_value = b;
        }
        if let Some(mask) = item.data_mask {
            data_value &= mask;
        }

        if item.operator.is_some() {
            let mut operator_type = item.operator;
            let mut operator_value: u8 = 0;
            if let Some(operator_index) = item.operator_index {
                match item.operator_data? {
                    OperatorDataType::RequestIdx => {
                        // C# checks `Count < index` and then throws on `Count == index`;
                        // treated as no match.
                        let request = request?;
                        operator_value = *request.get(operator_index as usize)?;
                    }
                    OperatorDataType::EcuAddr => {
                        let addr = sim_ecu_addr?;
                        operator_value = match operator_index {
                            0 => (addr & 0xFF) as u8,
                            1 => ((addr >> 8) & 0xFF) as u8,
                            _ => return None,
                        };
                    }
                    OperatorDataType::Checksum => {
                        let limit = operator_index as usize;
                        let bytes = out
                            .iter()
                            .enumerate()
                            .take_while(|(i, _)| limit == 0 || *i < limit);
                        match operator_type? {
                            OperatorType::Xor => {
                                for (_, b) in bytes {
                                    data_value ^= b;
                                }
                            }
                            OperatorType::Plus => {
                                for (_, b) in bytes {
                                    data_value = data_value.wrapping_add(*b);
                                }
                            }
                            _ => return None,
                        }
                        operator_type = None;
                    }
                }
            } else if let Some(v) = item.operator_value {
                operator_value = v;
            } else {
                return None;
            }

            if let Some(op) = operator_type {
                data_value = match op {
                    OperatorType::And => data_value & operator_value,
                    OperatorType::Or => data_value | operator_value,
                    OperatorType::Xor => data_value ^ operator_value,
                    OperatorType::Plus => data_value.wrapping_add(operator_value),
                    OperatorType::Minus => data_value.wrapping_sub(operator_value),
                    OperatorType::Multiply => data_value.wrapping_mul(operator_value),
                    OperatorType::Divide => {
                        if operator_value == 0 {
                            return None;
                        }
                        data_value / operator_value
                    }
                };
            }
        }
        out.push(data_value);
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// SimInterface (C#: EdInterfaceObd in simulation mode)
// ---------------------------------------------------------------------------

/// C#: `ParTransmitFunc`. Only BMW-FAST changes the simulation behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transmit {
    None,
    Ds2,
    Kwp1281,
    Concept3,
    Kwp2000Bmw,
    Kwp2000S,
    BmwFast,
}

/// OBD interface answering from a sim file.
pub struct SimInterface {
    /// C#: `EdSimFileInterface`.
    sim: SimFile,
    /// C#: `EdSimFileSgbd`, searched before the interface file.
    sgbd_sim: Option<SimFile>,
    /// C#: `SimulationConnected`.
    connected: bool,
    /// C#: `SimulationRecQueue` (BMW-FAST multi-telegram responses).
    rec_queue: VecDeque<Vec<u8>>,
    /// C#: `SimFrequentResponse`.
    frequent_response: Option<Vec<u8>>,
    sim_ecu_addr: Option<i32>,
    sim_wake_addr: Option<i32>,
    /// C#: `CommParameterProtected` (`None` = null).
    comm_par: Option<Vec<u32>>,
    comm_answer_len: [i16; 2],
    comm_repeats: u32,
    transmit: Transmit,
    has_key_bytes: bool,
    support_frequent: bool,
}

impl SimInterface {
    /// Loads the interface sim file. A file that does not parse is reported
    /// by `interface_connect` (EDIABAS_IFH_0026), like in C#.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<SimInterface> {
        Ok(SimInterface::new(SimFile::load(path)?))
    }

    pub fn new(sim: SimFile) -> SimInterface {
        SimInterface {
            sim,
            sgbd_sim: None,
            connected: false,
            rec_queue: VecDeque::new(),
            frequent_response: None,
            sim_ecu_addr: None,
            sim_wake_addr: None,
            comm_par: None,
            comm_answer_len: [0, 0],
            comm_repeats: 0,
            transmit: Transmit::None,
            has_key_bytes: false,
            support_frequent: false,
        }
    }

    pub fn sim_file(&self) -> &SimFile {
        &self.sim
    }

    /// C#: `LoadSgbdSimFile`. `path` is the SGBD sim file; a missing file is
    /// no error because the interface sim file is always present.
    pub fn load_sgbd_sim_file(&mut self, path: impl AsRef<Path>) -> IfResult<()> {
        self.unload_sgbd_sim_file();
        let path = path.as_ref();
        if !path.exists() {
            return Ok(());
        }
        match SimFile::load(path) {
            Ok(sim) if sim.file_valid() => {
                self.sgbd_sim = Some(sim);
                Ok(())
            }
            // C# returns false without an error code on a read exception.
            _ => Err(ErrorCode::EDIABAS_IFH_0026),
        }
    }

    /// C#: `UnloadSgbdSimFile`.
    pub fn unload_sgbd_sim_file(&mut self) {
        if self.sgbd_sim.is_some() {
            self.rec_queue.clear();
        }
        self.sgbd_sim = None;
        self.frequent_response = None;
    }

    /// C#: `UnloadInterfaceSimFile` (the parsed file is kept).
    fn unload_interface_sim(&mut self) {
        self.rec_queue.clear();
        self.frequent_response = None;
        self.sim_ecu_addr = None;
        self.connected = false;
    }

    /// C#: `CommParameter` setter, simulation subset (no timing, no EDIC).
    fn apply_comm_parameter(&mut self) -> IfResult<()> {
        self.comm_answer_len = [0, 0];
        self.transmit = Transmit::None;
        self.has_key_bytes = false;
        self.support_frequent = false;
        self.sim_ecu_addr = None;
        self.sim_wake_addr = None;

        let Some(p) = self.comm_par.clone() else {
            return Ok(());
        };
        if p.is_empty() {
            return Err(ErrorCode::EDIABAS_IFH_0041);
        }
        // HasAdapterEcho is false for a simulated adapter; EvalChecksumPar
        // always succeeds and only affects real transmissions.
        match p[0] {
            // Raw (EDIC): no EDIC simulation.
            0x0000 => return Err(ErrorCode::EDIABAS_IFH_0014),
            0x0001 => {
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.comm_answer_len = [-2, 0];
                self.transmit = Transmit::Ds2;
                self.support_frequent = true;
            }
            0x0002 => {
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.comm_answer_len = [1, 0];
                self.transmit = Transmit::Kwp1281;
                self.has_key_bytes = true;
                self.sim_wake_addr = Some((p[2] as u8) as i32);
            }
            0x0003 => {
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.comm_answer_len = [52, 0];
                self.transmit = Transmit::Concept3;
                self.has_key_bytes = true;
                self.support_frequent = true;
                self.sim_wake_addr = Some((p[2] as u8) as i32);
            }
            0x0005 | 0x0006 => {
                if p.len() < 8 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.comm_answer_len = [-1, 0];
                self.transmit = Transmit::Ds2;
                self.support_frequent = true;
            }
            0x010B => return Err(ErrorCode::EDIABAS_IFH_0014),
            0x010C => {
                if p.len() < 33 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.transmit = Transmit::Kwp2000Bmw;
                if p[9] > 11 || p[21] > 11 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.support_frequent = true;
            }
            0x010D => {
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.transmit = Transmit::Kwp2000S;
            }
            0x010F => {
                if p.len() < 7 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.transmit = Transmit::BmwFast;
            }
            0x0110 => {
                if p.len() < 30 {
                    return Err(ErrorCode::EDIABAS_IFH_0041);
                }
                self.transmit = Transmit::BmwFast;
            }
            _ => return Err(ErrorCode::EDIABAS_IFH_0014),
        }
        Ok(())
    }

    /// C#: `TransmitSimulationData`. `Err` = C# set an error code, `Ok(None)` =
    /// C# returned false without an error.
    fn transmit_simulation_data(
        &mut self,
        send: &[u8],
        bmw_fast: bool,
    ) -> IfResult<Option<Vec<u8>>> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        if !bmw_fast {
            self.rec_queue.clear();
            return Ok(self.transmit_simulation_internal(send));
        }

        if !send.is_empty() {
            self.rec_queue.clear();
            let Some(mut rec) = self.transmit_simulation_internal(send) else {
                return Ok(None);
            };
            loop {
                let tel_length = match tel_length_bmw_fast(&rec) {
                    Some(0) | None => return Ok(None),
                    Some(l) => l,
                };
                if tel_length + 1 > rec.len() {
                    return Ok(None);
                }
                // Including checksum.
                let mut response: Vec<u8> = rec[..tel_length + 1].to_vec();
                // Drop "busy" / "response pending" negative responses.
                let filter = response.len() == 7
                    && response[3] == 0x7F
                    && matches!(response[5], 0x22 | 0x23 | 0x78);
                if !filter {
                    let last = response.len() - 1;
                    response[last] = calc_checksum_bmw_fast(&response, tel_length);
                    self.rec_queue.push_back(response);
                }
                rec.drain(..tel_length + 1);
                if rec.is_empty() {
                    break;
                }
            }
        }
        Ok(self.rec_queue.pop_front())
    }

    /// C#: `TransmitSimulationInternal`. SGBD file first, then the interface
    /// file; each with the address first, then without.
    fn transmit_simulation_internal(&mut self, send: &[u8]) -> Option<Vec<u8>> {
        let (ecu, wake) = (self.sim_ecu_addr, self.sim_wake_addr);
        let has_addr = ecu.is_some() || wake.is_some();
        for sim in self
            .sgbd_sim
            .iter_mut()
            .chain(std::iter::once(&mut self.sim))
        {
            let mut response = sim.get_response(send, ecu, wake);
            if response.is_none() && has_addr {
                response = sim.get_response(send, None, None);
            }
            if response.is_some() {
                return response;
            }
        }
        None
    }

    /// C#: `GetKeyBytesSimulation`. Quirk: tries without address first.
    fn key_bytes_simulation(&mut self) -> IfResult<Option<Vec<u8>>> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        let (ecu, wake) = (self.sim_ecu_addr, self.sim_wake_addr);
        let has_addr = ecu.is_some() || wake.is_some();
        for sim in self
            .sgbd_sim
            .iter_mut()
            .chain(std::iter::once(&mut self.sim))
        {
            let mut key_bytes = sim.get_key_bytes(None, None);
            if key_bytes.is_none() && has_addr {
                key_bytes = sim.get_key_bytes(ecu, wake);
            }
            if key_bytes.is_some() {
                return Ok(key_bytes);
            }
        }
        Ok(None)
    }
}

impl Interface for SimInterface {
    fn interface_type(&self) -> &str {
        "OBD"
    }

    fn interface_version(&self) -> u32 {
        209
    }

    fn connected(&self) -> bool {
        self.connected
    }

    /// C#: base `InterfaceConnect` + `LoadInterfaceSimFile`. C# re-reads the
    /// file, which resets all response indices.
    fn interface_connect(&mut self) -> IfResult<()> {
        self.comm_repeats = 0;
        // C# sets the field, not the property: transmit mode etc. stay.
        self.comm_par = None;
        self.comm_answer_len = [0, 0];
        self.unload_interface_sim();
        self.sim.reset_all();
        if !self.sim.file_valid() {
            return Err(ErrorCode::EDIABAS_IFH_0026);
        }
        self.connected = true;
        Ok(())
    }

    fn interface_disconnect(&mut self) -> IfResult<()> {
        self.unload_interface_sim();
        Ok(())
    }

    fn interface_reset(&mut self) -> IfResult<()> {
        let connected = self.connected;
        self.comm_par = None;
        self.apply_comm_parameter()?;
        if !connected {
            // C# sets the error, clears the parameters and still returns true.
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(())
    }

    fn interface_boot(&mut self) -> IfResult<()> {
        self.comm_par = None;
        self.apply_comm_parameter()
    }

    fn transmit_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>> {
        if self.comm_par.is_none() {
            return Err(ErrorCode::EDIABAS_IFH_0006);
        }
        if request.len() > TRANS_BUFFER_SIZE {
            return Err(ErrorCode::EDIABAS_IFH_0031);
        }
        // C# sets IFH_0009 after an inner error too; the inner (first) one is
        // returned here, which is what raises when untrapped.
        match self.transmit_simulation_data(request, self.transmit == Transmit::BmwFast)? {
            Some(r) => Ok(r),
            None => Err(ErrorCode::EDIABAS_IFH_0009),
        }
    }

    fn transmit_frequent(&mut self, request: &[u8]) -> IfResult<()> {
        if self.comm_par.is_none() || !self.support_frequent || request.is_empty() {
            return Err(ErrorCode::EDIABAS_IFH_0006);
        }
        self.frequent_response = None;
        // C# returns true even without simulation data.
        self.frequent_response = self.transmit_simulation_data(request, false)?;
        Ok(())
    }

    fn receive_frequent(&mut self) -> IfResult<Vec<u8>> {
        if self.comm_par.is_none() {
            return Err(ErrorCode::EDIABAS_IFH_0006);
        }
        if !self.support_frequent {
            return Ok(Vec::new());
        }
        Ok(self.frequent_response.clone().unwrap_or_default())
    }

    fn stop_frequent(&mut self) -> IfResult<()> {
        self.frequent_response = None;
        Ok(())
    }

    fn raw_data(&mut self, request: &[u8]) -> IfResult<Vec<u8>> {
        if request == [0xF1, 0x07] {
            return Ok(vec![0x05, 0x03, 0x00]);
        }
        Ok(Vec::new())
    }

    fn comm_parameter(&self) -> &[u32] {
        self.comm_par.as_deref().unwrap_or(&[])
    }

    fn set_comm_parameter(&mut self, pars: Vec<u32>) -> IfResult<()> {
        self.comm_par = Some(pars);
        self.apply_comm_parameter()
    }

    fn comm_answer_len(&self) -> &[i16] {
        &self.comm_answer_len
    }

    fn set_comm_answer_len(&mut self, len: Vec<i16>) -> IfResult<()> {
        if len.len() >= 2 {
            self.comm_answer_len = [len[0], len[1]];
        }
        Ok(())
    }

    fn comm_repeats(&self) -> u32 {
        self.comm_repeats
    }

    fn set_comm_repeats(&mut self, repeats: u32) {
        self.comm_repeats = repeats;
    }

    fn key_bytes(&mut self) -> IfResult<Vec<u8>> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        // C# falls back to KeyBytesProtected, which is only set for EDIC.
        if self.comm_par.is_some()
            && self.has_key_bytes
            && let Some(k) = self.key_bytes_simulation()?
        {
            return Ok(k);
        }
        Ok(Vec::new())
    }

    /// C# reads the DSR line (KL15). Without hardware the sim file's
    /// ignition voltage decides: > 0 is KL15 on (0x00), else off (0x30).
    fn state(&mut self) -> IfResult<Vec<u8>> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(vec![
            0x00,
            if self.sim.ignition_volt > 0 {
                0x00
            } else {
                0x30
            },
        ])
    }

    fn loop_test(&mut self) -> IfResult<u32> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(1)
    }

    /// C#: `UbatVoltageSimulation`.
    fn battery_voltage(&mut self) -> IfResult<i64> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(self.sim.ubat_volt as i64)
    }

    /// C#: `IgnitionVoltageSimulation`.
    fn ignition_voltage(&mut self) -> IfResult<i64> {
        if !self.connected {
            return Err(ErrorCode::EDIABAS_IFH_0056);
        }
        Ok(self.sim.ignition_volt as i64)
    }

    fn get_port(&mut self, _index: u32) -> IfResult<i64> {
        Ok(0)
    }

    fn set_port(&mut self, _index: u32, _value: u32) -> IfResult<()> {
        Ok(())
    }

    fn set_program_voltage(&mut self, _voltage: u32) -> IfResult<()> {
        Ok(())
    }

    fn switch_si_relais(&mut self, _time: u32) -> IfResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic telegrams (ECU 0x40), not taken from any real sim file.
    const SIM: &str = "\
; synthetic test file
[POWERSUPPLY]
UBatt = 12800

[IGNITION]
Ignition = 11900

[REQUEST]
read_0=83,40,F1,22,10,00
read_1=83,40,F1,22,10,00
wild_0=83,40,F1,22,X0,0X
exact_0=83,40,F1,22,20,00
var_0=83,40,F1,31,..
echo_0=82,40,F1,21,XX
pend_0=82,40,F1,30,01
two_0=82,40,F1,30,02
ds2_0=40,05,0B,03
calc_0=82,40,F1,21,FF

[RESPONSE]
read_0=83,F1,40,62,10,01,00
read_1=83,F1,40,62,10,02,00
wild_0=82,F1,40,62,AA,00
exact_0=82,F1,40,62,BB,00
var_0=82,F1,40,71,00+[4],00
echo_0=82,F1,40,61,00+[4],00
pend_0=83,F1,40,7F,30,78,00,83,F1,40,70,01,02,00
two_0=82,F1,40,70,01,00,82,F1,40,70,02,00
ds2_0=40,06,A0,00,00^$0
calc_0=84,F1,40,61,FF+01,10-01,20*2,00

[12.REQUEST]
k_0=01,02
k2_0=01,03

[12.RESPONSE]
k_0=03,00+#0
k2_0=03,04

[12.KEYBYTES]
a=01,8A
b=02,8A
";

    fn bmw_fast() -> SimInterface {
        let mut s = SimInterface::new(SimFile::parse(SIM));
        s.interface_connect().unwrap();
        s.set_comm_parameter(vec![0x010F, 115200, 1000, 20, 20, 3, 5000])
            .unwrap();
        s
    }

    fn checksum_ok(t: &[u8]) -> bool {
        let n = t.len() - 1;
        calc_checksum_bmw_fast(t, n) == t[n]
    }

    #[test]
    fn parse_power_and_validity() {
        let sim = SimFile::parse(SIM);
        assert!(sim.file_valid());
        assert_eq!(sim.ubat_volt, 12800);
        assert_eq!(sim.ignition_volt, 11900);
        assert_eq!(sim.ubat_current, -1);
        let empty = SimFile::parse("[REQUEST]\n");
        assert!(empty.file_valid());
        assert_eq!(empty.ubat_volt, DEFAULT_BATTERY_VOLT);
    }

    #[test]
    fn missing_response_is_invalid() {
        let sim = SimFile::parse("[REQUEST]\na=01\n[RESPONSE]\nb=02\n");
        assert!(!sim.file_valid());
        let mut s = SimInterface::new(sim);
        assert_eq!(s.interface_connect(), Err(ErrorCode::EDIABAS_IFH_0026));
        assert!(!s.connected());
    }

    #[test]
    fn connect_sets_connected() {
        let mut s = SimInterface::new(SimFile::parse(SIM));
        assert!(!s.connected());
        assert_eq!(s.transmit_data(&[0x01]), Err(ErrorCode::EDIABAS_IFH_0006));
        s.interface_connect().unwrap();
        assert!(s.connected());
        assert_eq!(s.interface_type(), "OBD");
        assert_eq!(s.battery_voltage(), Ok(12800));
        assert_eq!(s.ignition_voltage(), Ok(11900));
        s.interface_disconnect().unwrap();
        assert!(!s.connected());
        assert_eq!(s.battery_voltage(), Err(ErrorCode::EDIABAS_IFH_0056));
    }

    #[test]
    fn repeat_keys_cycle_and_checksum_is_fixed() {
        let mut s = bmw_fast();
        let req = [0x83, 0x40, 0xF1, 0x22, 0x10, 0x00];
        let r1 = s.transmit_data(&req).unwrap();
        let r2 = s.transmit_data(&req).unwrap();
        let r3 = s.transmit_data(&req).unwrap();
        assert_eq!(&r1[..6], &[0x83, 0xF1, 0x40, 0x62, 0x10, 0x01]);
        assert_eq!(r2[5], 0x02);
        assert_eq!(r3[5], 0x01);
        assert!(checksum_ok(&r1) && checksum_ok(&r2));
        // Reconnect resets the index.
        s.interface_connect().unwrap();
        s.set_comm_parameter(vec![0x010F, 115200, 1000, 20, 20, 3, 5000])
            .unwrap();
        assert_eq!(s.transmit_data(&req).unwrap()[5], 0x01);
    }

    #[test]
    fn exact_match_wins_over_wildcard() {
        let mut s = bmw_fast();
        // 20 00 matches both `exact` and `X0,0X`; exact entries are tried first.
        assert_eq!(
            s.transmit_data(&[0x83, 0x40, 0xF1, 0x22, 0x20, 0x00])
                .unwrap()[4],
            0xBB
        );
        assert_eq!(
            s.transmit_data(&[0x83, 0x40, 0xF1, 0x22, 0x30, 0x05])
                .unwrap()[4],
            0xAA
        );
        // Low nibble of byte 4 must be 0.
        assert_eq!(
            s.transmit_data(&[0x83, 0x40, 0xF1, 0x22, 0x31, 0x00]),
            Err(ErrorCode::EDIABAS_IFH_0009)
        );
    }

    #[test]
    fn variable_length_and_request_operand() {
        let mut s = bmw_fast();
        let r = s
            .transmit_data(&[0x83, 0x40, 0xF1, 0x31, 0x5A, 0x01])
            .unwrap();
        assert_eq!(&r[..5], &[0x82, 0xF1, 0x40, 0x71, 0x5A]);
        let r = s.transmit_data(&[0x82, 0x40, 0xF1, 0x21, 0x07]).unwrap();
        assert_eq!(&r[..5], &[0x82, 0xF1, 0x40, 0x61, 0x07]);
        // Shorter than the fixed part: no match.
        assert_eq!(
            s.transmit_data(&[0x83, 0x40, 0xF1]),
            Err(ErrorCode::EDIABAS_IFH_0009)
        );
    }

    #[test]
    fn constant_operators() {
        let mut s = bmw_fast();
        // 82 40 F1 21 FF: exact `calc` entry beats the XX wildcard.
        let r = s.transmit_data(&[0x82, 0x40, 0xF1, 0x21, 0xFF]).unwrap();
        assert_eq!(&r[4..7], &[0x00, 0x0F, 0x40]);
    }

    #[test]
    fn bmw_fast_filters_pending_and_queues_telegrams() {
        let mut s = bmw_fast();
        let r = s.transmit_data(&[0x82, 0x40, 0xF1, 0x30, 0x01]).unwrap();
        assert_eq!(&r[..6], &[0x83, 0xF1, 0x40, 0x70, 0x01, 0x02]);
        assert!(checksum_ok(&r));

        let r = s.transmit_data(&[0x82, 0x40, 0xF1, 0x30, 0x02]).unwrap();
        assert_eq!(r[4], 0x01);
        // Empty request fetches the next queued telegram.
        let r = s.transmit_data(&[]).unwrap();
        assert_eq!(r[4], 0x02);
        assert!(checksum_ok(&r));
        assert_eq!(s.transmit_data(&[]), Err(ErrorCode::EDIABAS_IFH_0009));
    }

    #[test]
    fn ds2_xor_checksum_operator() {
        let mut s = SimInterface::new(SimFile::parse(SIM));
        s.interface_connect().unwrap();
        s.set_comm_parameter(vec![0x0006, 9600, 0, 0, 0, 0, 0, 0])
            .unwrap();
        let r = s.transmit_data(&[0x40, 0x05, 0x0B, 0x03]).unwrap();
        assert_eq!(r, vec![0x40, 0x06, 0xA0, 0x00, 0x40 ^ 0x06 ^ 0xA0]);
    }

    #[test]
    fn wake_address_sections_and_key_bytes() {
        let mut s = SimInterface::new(SimFile::parse(SIM));
        s.interface_connect().unwrap();
        // Concept 2 (KWP1281), wake address 0x12.
        s.set_comm_parameter(vec![0x0002, 10400, 0x12, 0, 0, 0, 0])
            .unwrap();
        assert_eq!(s.transmit_data(&[0x01, 0x03]).unwrap(), vec![0x03, 0x04]);
        // `#0` needs the ECU address; with only a wake address there is no answer.
        assert_eq!(
            s.transmit_data(&[0x01, 0x02]),
            Err(ErrorCode::EDIABAS_IFH_0009)
        );
        assert_eq!(s.key_bytes().unwrap(), vec![0x01, 0x8A]);
        assert_eq!(s.key_bytes().unwrap(), vec![0x02, 0x8A]);
        assert_eq!(s.key_bytes().unwrap(), vec![0x01, 0x8A]);
        // Without address the [12.] entries are not visible.
        s.set_comm_parameter(vec![0x010F, 115200, 1000, 20, 20, 3, 5000])
            .unwrap();
        assert_eq!(
            s.transmit_data(&[0x01, 0x02]),
            Err(ErrorCode::EDIABAS_IFH_0009)
        );
        assert_eq!(s.key_bytes().unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn ecu_addr_operand() {
        let mut sim = SimFile::parse(SIM);
        // `#0` uses the ECU address, not the wake address.
        assert_eq!(
            sim.get_response(&[0x01, 0x02], Some(0x12), None),
            Some(vec![0x03, 0x12])
        );
        assert_eq!(sim.get_response(&[0x01, 0x02], None, Some(0x12)), None);
    }

    #[test]
    fn comm_parameter_errors() {
        let mut s = SimInterface::new(SimFile::parse(SIM));
        s.interface_connect().unwrap();
        assert_eq!(
            s.set_comm_parameter(vec![]),
            Err(ErrorCode::EDIABAS_IFH_0041)
        );
        assert_eq!(
            s.set_comm_parameter(vec![0x010F, 1]),
            Err(ErrorCode::EDIABAS_IFH_0041)
        );
        assert_eq!(
            s.set_comm_parameter(vec![0x1234]),
            Err(ErrorCode::EDIABAS_IFH_0014)
        );
        // Parameters stay set, like the C# field.
        assert_eq!(s.comm_parameter(), &[0x1234]);
        s.interface_boot().unwrap();
        assert!(s.comm_parameter().is_empty());
    }

    #[test]
    fn frequent_and_raw() {
        let mut s = SimInterface::new(SimFile::parse(SIM));
        s.interface_connect().unwrap();
        s.set_comm_parameter(vec![0x0006, 9600, 0, 0, 0, 0, 0, 0])
            .unwrap();
        s.transmit_frequent(&[0x40, 0x05, 0x0B, 0x03]).unwrap();
        assert_eq!(s.receive_frequent().unwrap().len(), 5);
        s.stop_frequent().unwrap();
        assert!(s.receive_frequent().unwrap().is_empty());
        assert_eq!(s.raw_data(&[0xF1, 0x07]).unwrap(), vec![0x05, 0x03, 0x00]);
        assert_eq!(s.state().unwrap(), vec![0x00, 0x00]);
        assert_eq!(s.loop_test(), Ok(1));
    }

    #[test]
    fn parse_hex_string_rules() {
        assert!(parse_hex_string("_", true).unwrap().is_empty());
        assert!(parse_hex_string("..", true).is_none());
        assert!(parse_hex_string("1", true).is_none());
        assert!(parse_hex_string("12?3", true).is_none());
        assert!(parse_hex_string("12+[", true).is_none());
        let items = parse_hex_string("x1, 2X ,00^$3", true).unwrap();
        assert_eq!(
            (items[0].data_value, items[0].data_mask),
            (0x01, Some(0x0F))
        );
        assert_eq!(
            (items[1].data_value, items[1].data_mask),
            (0x20, Some(0xF0))
        );
        assert_eq!(items[2].operator_data, Some(OperatorDataType::Checksum));
        assert_eq!(items[2].operator_index, Some(3));
        // Wildcards are literal in responses ("X1" does not parse as hex).
        assert!(parse_hex_string("X1", false).is_none());
    }

    /// Reads a real sim file from `BMWA_SIM_FILE` (not part of the repo).
    #[test]
    fn env_sim_file_dme_ident() {
        let Ok(path) = std::env::var("BMWA_SIM_FILE") else {
            eprintln!("BMWA_SIM_FILE not set, skipping");
            return;
        };
        let mut s = SimInterface::open(&path).expect("read sim file");
        assert!(s.sim_file().file_valid());
        s.interface_connect().unwrap();
        s.set_comm_parameter(vec![0x010F, 115200, 1000, 20, 20, 3, 5000])
            .unwrap();
        // BMW-FAST requests are sent without checksum; the response checksum
        // is recalculated by the interface.
        let r = s.transmit_data(&[0x82, 0x12, 0xF1, 0x1A, 0x80]).unwrap();
        assert!(!r.is_empty());
        assert_eq!(&r[1..5], &[0xF1, 0x12, 0x5A, 0x80]);
        assert!(checksum_ok(&r));
    }
}
