//! Diagnostics worker. Owns the BEST/2 runtime on its own thread: the UI sends
//! `Command`s and drains `Event`s, so a scan never blocks drawing.
//! `FaultMemory` is the UI-side model the events are folded into.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use best::{Ediabas, ResultSet, ResultValue};

use crate::Args;
use crate::modules::{self, Module};

const JOB_READ_FAULTS: &str = "FS_LESEN";
const JOB_CLEAR_FAULTS: &str = "FS_LOESCHEN";
const F_ORT_NR: &str = "F_ORT_NR";
const NO_ADAPTER: &str = "No adapter connected. Plug in the cable, or start with --sim <file>.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Fault location number, shown as hex.
    pub code: u32,
    pub text: String,
    pub symptom: String,
    /// Stored but not active right now (F_VORHANDEN_TEXT).
    pub present: String,
    pub warning: String,
}

#[derive(Debug, Clone)]
pub enum Command {
    Scan,
    ReadFaults(String),
    ClearFaults(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// `done` of `total` modules checked. `done == total` ends the scan.
    ScanProgress { done: usize, total: usize, current_name: String },
    ModuleFound { id: String, variant: String, fault_count: usize },
    ModuleMissing { id: String },
    Faults { id: String, faults: Vec<Fault> },
    Cleared { id: String, ok: bool, message: String },
    /// `id` is empty for errors not tied to one module.
    Error { id: String, message: String },
}

/// UI side of the worker: send commands, drain events.
pub struct Handle {
    pub tx: Sender<Command>,
    pub rx: Receiver<Event>,
}

/// Starts the worker. It builds its own `Ediabas` and interface, because
/// interfaces are not `Send`.
pub fn spawn(ecu_dir: PathBuf, args: Args) -> Handle {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut ed = Ediabas::new(ecu_dir);
        if let Some(iface) = crate::make_interface(&args) {
            ed.set_interface(iface);
        }
        for cmd in cmd_rx {
            let alive = match cmd {
                Command::Scan => scan(&mut ed, &ev_tx),
                Command::ReadFaults(id) => read_faults(&mut ed, &id, &ev_tx),
                Command::ClearFaults(id) => clear_faults(&mut ed, &id, &ev_tx),
            };
            if !alive {
                break;
            }
        }
    });
    Handle { tx: cmd_tx, rx: ev_rx }
}

/// Returns false when the UI side is gone.
fn emit(tx: &Sender<Event>, ev: Event) -> bool {
    tx.send(ev).is_ok()
}

/// Modules whose group file exists in the ECU directory, in list order.
fn available(dir: &Path) -> Vec<&'static Module> {
    let names: HashSet<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_lowercase())
        .collect();
    modules::MODULES.iter().filter(|m| names.contains(&format!("{}.grp", m.id))).collect()
}

/// Resolves the module's group file and runs one job on it.
fn run_job(ed: &mut Ediabas, id: &str, job: &str) -> Result<Vec<ResultSet>, String> {
    if ed.iface_mut().is_none() {
        return Err(NO_ADAPTER.into());
    }
    ed.resolve_sgbd(id).map_err(|e| e.to_string())?;
    ed.execute_job(job, "", "").map_err(|e| e.to_string())
}

fn scan(ed: &mut Ediabas, tx: &Sender<Event>) -> bool {
    let list = available(ed.ecu_dir());
    let total = list.len();
    if ed.iface_mut().is_none() {
        let progress = Event::ScanProgress { done: 0, total: 0, current_name: String::new() };
        let error = Event::Error { id: String::new(), message: NO_ADAPTER.into() };
        return emit(tx, progress) && emit(tx, error);
    }
    for (done, m) in list.iter().enumerate() {
        if !emit(tx, Event::ScanProgress { done, total, current_name: m.name_en.into() }) {
            return false;
        }
        let ev = match run_job(ed, m.id, JOB_READ_FAULTS) {
            Ok(sets) => {
                let first = sets.first().map(|s| text(s, "VARIANTE")).unwrap_or_default();
                let variant = if first.is_empty() { ed.variant_name() } else { first };
                Event::ModuleFound { id: m.id.into(), variant, fault_count: parse_faults(&sets).len() }
            }
            Err(_) => Event::ModuleMissing { id: m.id.into() },
        };
        if !emit(tx, ev) {
            return false;
        }
    }
    emit(tx, Event::ScanProgress { done: total, total, current_name: String::new() })
}

fn read_faults(ed: &mut Ediabas, id: &str, tx: &Sender<Event>) -> bool {
    let ev = match run_job(ed, id, JOB_READ_FAULTS) {
        Ok(sets) => Event::Faults { id: id.into(), faults: parse_faults(&sets) },
        Err(message) => Event::Error { id: id.into(), message },
    };
    emit(tx, ev)
}

fn clear_faults(ed: &mut Ediabas, id: &str, tx: &Sender<Event>) -> bool {
    let ev = match run_job(ed, id, JOB_CLEAR_FAULTS) {
        Ok(sets) => {
            let status = sets.iter().rev().map(|s| text(s, "JOB_STATUS")).find(|t| !t.is_empty()).unwrap_or_default();
            let ok = status.is_empty() || status == "OKAY";
            Event::Cleared { id: id.into(), ok, message: if ok { String::new() } else { status } }
        }
        Err(message) => Event::Cleared { id: id.into(), ok: false, message },
    };
    emit(tx, ev)
}

/// Faults are the result sets after set 0 that carry `F_ORT_NR`.
pub fn parse_faults(sets: &[ResultSet]) -> Vec<Fault> {
    sets.iter()
        .skip(1)
        .filter(|s| s.get(F_ORT_NR).is_some())
        .map(|s| Fault {
            code: match s.get(F_ORT_NR).map(|d| &d.value) {
                Some(ResultValue::Int(v)) => u32::try_from(*v).unwrap_or(0),
                _ => 0,
            },
            text: text(s, "F_ORT_TEXT"),
            symptom: text(s, "F_SYMPTOM_TEXT"),
            present: text(s, "F_VORHANDEN_TEXT"),
            warning: text(s, "F_WARNUNG_TEXT"),
        })
        .collect()
}

/// String value of a result, or "" when missing.
fn text(set: &ResultSet, name: &str) -> String {
    match set.get(name).map(|d| &d.value) {
        Some(ResultValue::Str(s)) => s.trim().to_string(),
        Some(ResultValue::Int(v)) => v.to_string(),
        _ => String::new(),
    }
}

pub fn code_hex(code: u32) -> String {
    format!("0x{code:04X}")
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ScanState {
    #[default]
    Idle,
    Running { done: usize, total: usize, current: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    #[cfg_attr(not(test), allow(dead_code))]
    pub variant: String,
    pub fault_count: usize,
}

/// UI-side model: scan result, per-module faults, read errors.
#[derive(Debug, Default)]
pub struct FaultMemory {
    pub scan: ScanState,
    /// Found modules, in `modules::MODULES` order.
    pub found: Vec<Found>,
    /// Ids of modules that did not answer in the last scan.
    pub silent: Vec<String>,
    pub faults: HashMap<String, Vec<Fault>>,
    pub errors: HashMap<String, String>,
    /// Ids with a read in flight or done, so reads are not repeated every frame.
    pub requested: HashSet<String>,
    pub notice: Option<String>,
}

impl FaultMemory {
    /// Clears the previous result and enters the running state.
    pub fn begin_scan(&mut self) {
        *self = Self { scan: ScanState::Running { done: 0, total: 0, current: String::new() }, ..Self::default() };
    }

    pub fn needs_read(&self, id: &str) -> bool {
        !self.faults.contains_key(id) && !self.errors.contains_key(id) && !self.requested.contains(id)
    }

    pub fn apply(&mut self, ev: Event) {
        match ev {
            Event::ScanProgress { done, total, current_name } => {
                if done >= total {
                    self.scan = ScanState::Idle;
                    self.notice = Some(format!(
                        "Scan done: {} found, {} not answering.",
                        self.found.len(),
                        self.silent.len()
                    ));
                } else {
                    self.scan = ScanState::Running { done, total, current: current_name };
                }
            }
            Event::ModuleFound { id, variant, fault_count } => {
                self.found.push(Found { id, variant, fault_count });
                self.found.sort_by_key(|f| modules::position(&f.id));
            }
            Event::ModuleMissing { id } => self.silent.push(id),
            Event::Faults { id, faults } => {
                if let Some(f) = self.found.iter_mut().find(|f| f.id == id) {
                    f.fault_count = faults.len();
                }
                self.errors.remove(&id);
                self.faults.insert(id, faults);
            }
            Event::Cleared { id, ok, message } => {
                if ok {
                    self.faults.remove(&id);
                    self.errors.remove(&id);
                    self.requested.remove(&id);
                    self.notice = Some("Stored faults deleted.".into());
                } else {
                    self.notice = Some(message);
                }
            }
            Event::Error { id, message } => {
                if !id.is_empty() {
                    self.errors.insert(id, message.clone());
                }
                self.notice = Some(message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use best::{ResultData, ResultType};

    use super::*;

    fn set(items: &[(&str, ResultValue)]) -> ResultSet {
        let mut s = ResultSet::default();
        for (name, value) in items {
            s.set(ResultData::new(ResultType::S, *name, value.clone()));
        }
        s
    }

    fn s(v: &str) -> ResultValue {
        ResultValue::Str(v.into())
    }

    #[test]
    fn parses_faults_from_result_sets() {
        let system = set(&[("VARIANTE", s("PDC_65_2")), ("F_ORT_NR", ResultValue::Int(1))]);
        let fault = set(&[
            ("F_ORT_NR", ResultValue::Int(0x2A8F)),
            ("F_ORT_TEXT", s("Fehlerort: CAN-High")),
            ("F_SYMPTOM_TEXT", s("kein Signal oder Wert")),
            ("F_VORHANDEN_TEXT", s("Fehler momentan nicht vorhanden")),
            ("F_WARNUNG_TEXT", s("Keine Warnlampe")),
        ]);
        let trailer = set(&[("JOB_STATUS", s("OKAY"))]);
        let bare = set(&[("F_ORT_NR", ResultValue::Int(0xE205))]);

        let faults = parse_faults(&[system, fault, trailer, bare]);

        assert_eq!(faults.len(), 2);
        assert_eq!(faults[0].code, 0x2A8F);
        assert_eq!(code_hex(faults[0].code), "0x2A8F");
        assert_eq!(faults[0].text, "Fehlerort: CAN-High");
        assert_eq!(faults[0].symptom, "kein Signal oder Wert");
        assert_eq!(faults[0].present, "Fehler momentan nicht vorhanden");
        assert_eq!(faults[0].warning, "Keine Warnlampe");
        assert_eq!(code_hex(faults[1].code), "0xE205");
        assert_eq!(faults[1].text, "");
        assert_eq!(faults[1].present, "");
    }

    #[test]
    fn no_fault_sets_means_no_faults() {
        assert!(parse_faults(&[set(&[("VARIANTE", s("X"))])]).is_empty());
        assert!(parse_faults(&[]).is_empty());
    }

    #[test]
    fn scan_events_fold_into_memory() {
        let mut m = FaultMemory::default();
        m.begin_scan();
        assert!(matches!(m.scan, ScanState::Running { .. }));

        m.apply(Event::ScanProgress { done: 0, total: 3, current_name: "Engine".into() });
        assert_eq!(
            m.scan,
            ScanState::Running { done: 0, total: 3, current: "Engine".into() }
        );
        m.apply(Event::ModuleFound { id: "d_pdc".into(), variant: "pdc".into(), fault_count: 1 });
        m.apply(Event::ModuleMissing { id: "d_egs".into() });
        m.apply(Event::ModuleFound { id: "d_motor".into(), variant: "d60".into(), fault_count: 0 });

        // Found modules keep scan-list order, not arrival order.
        let ids: Vec<&str> = m.found.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["d_motor", "d_pdc"]);

        m.apply(Event::ScanProgress { done: 3, total: 3, current_name: String::new() });
        assert_eq!(m.scan, ScanState::Idle);
        assert_eq!(m.notice.as_deref(), Some("Scan done: 2 found, 1 not answering."));
    }

    #[test]
    fn begin_scan_resets_previous_result() {
        let mut m = FaultMemory::default();
        m.apply(Event::ModuleFound { id: "d_motor".into(), variant: "d60".into(), fault_count: 0 });
        m.faults.insert("d_motor".into(), Vec::new());
        m.begin_scan();
        assert!(m.found.is_empty());
        assert!(m.faults.is_empty());
    }

    #[test]
    fn fault_reads_and_clear_flow() {
        let mut m = FaultMemory::default();
        m.apply(Event::ModuleFound { id: "d_pdc".into(), variant: "pdc".into(), fault_count: 1 });
        assert!(m.needs_read("d_pdc"));
        m.requested.insert("d_pdc".into());
        assert!(!m.needs_read("d_pdc"));

        let fault = Fault { code: 0xE205, text: "t".into(), symptom: String::new(), present: String::new(), warning: String::new() };
        m.apply(Event::Faults { id: "d_pdc".into(), faults: vec![fault] });
        assert_eq!(m.found[0].fault_count, 1);
        assert!(!m.needs_read("d_pdc"));

        m.apply(Event::Cleared { id: "d_pdc".into(), ok: false, message: "locked".into() });
        assert_eq!(m.notice.as_deref(), Some("locked"));
        assert!(!m.needs_read("d_pdc"));

        m.apply(Event::Cleared { id: "d_pdc".into(), ok: true, message: String::new() });
        assert!(m.faults.is_empty());
        assert!(m.needs_read("d_pdc"));
    }

    #[test]
    fn read_error_is_kept_per_module() {
        let mut m = FaultMemory::default();
        m.apply(Event::Error { id: "d_egs".into(), message: "no answer".into() });
        assert_eq!(m.errors.get("d_egs").map(String::as_str), Some("no answer"));
        assert!(!m.needs_read("d_egs"));
        m.apply(Event::Error { id: String::new(), message: NO_ADAPTER.into() });
        assert!(!m.errors.contains_key(""));
    }

    /// Needs a real ECU directory and a simulation file, e.g.
    /// BMWA_ECU_DIR=.../EcuRenamed BMWA_SIM_FILE=.../Sample/obd.sim cargo test -p bmwa
    #[test]
    fn worker_scans_sim_and_finds_motor() {
        let (Ok(dir), Ok(sim)) = (std::env::var("BMWA_ECU_DIR"), std::env::var("BMWA_SIM_FILE")) else {
            eprintln!("skipped: set BMWA_ECU_DIR and BMWA_SIM_FILE");
            return;
        };
        let args = Args { ecu_dir: PathBuf::from(&dir), sim: Some(PathBuf::from(sim)), port: None };
        let handle = spawn(PathBuf::from(&dir), args);
        handle.tx.send(Command::Scan).unwrap();

        let mut motor = None;
        loop {
            match handle.rx.recv_timeout(Duration::from_secs(120)).expect("worker timed out") {
                Event::ModuleFound { id, variant, .. } if id == "d_motor" => motor = Some(variant),
                Event::ScanProgress { done, total, .. } if done == total => break,
                _ => {}
            }
        }
        let variant = motor.expect("d_motor not found");
        assert!(!variant.is_empty());
    }
}
