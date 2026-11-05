//! Job execution layer (C#: `EdiabasNet.ResolveSgbdFile`, `ExecuteJob`,
//! `ExecuteJobPrivate`, `ExecuteInitJob`, `ExecuteExitJob`,
//! `ExecuteIdentJob`, `ReadAllJobs`, `CreateSystemResultDict`).
//!
//! `Ediabas` owns one `Vm` for its whole life, like the C# `EdiabasNet`
//! object owns its registers and config. Loading another SGBD only swaps
//! `vm.sgbd`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sgbd::Sgbd;

use crate::iface::Interface;
use crate::result::{ResultData, ResultSet, ResultType, ResultValue};
use crate::vm::Vm;
use crate::{ErrorCode, VmError, VmResult};

/// C#: `EdiabasVersion`.
pub const EDIABAS_VERSION: u32 = 0x773;
const PRG_FILE_EXT: &str = ".prg";
const GROUP_FILE_EXT: &str = ".grp";
const JOB_NAME_INIT: &str = "INITIALISIERUNG";
const JOB_NAME_EXIT: &str = "ENDE";
const JOB_NAME_IDENT: &str = "IDENTIFIKATION";

/// C#: `IsMinVersion760`.
const IS_MIN_VERSION_760: bool = EDIABAS_VERSION >= 0x760;

/// C#: `JobInfo`. `uses` is set for jobs inherited from a `uses` file
/// (C#: `UsesInfo`); the bytecode then lives in that file.
#[derive(Clone)]
struct JobInfo {
    name: String,
    address: u32,
    uses: Option<UsesInfo>,
}

#[derive(Clone)]
struct UsesInfo {
    /// Name as stored in the main file's uses list.
    name: String,
    sgbd: Arc<Sgbd>,
}

/// C#: `VariantInfo` in `_groupMappingDict`.
#[derive(Clone)]
struct VariantInfo {
    variant_name: String,
    family_name: String,
}

pub struct Ediabas {
    /// C#: `EcuPath`.
    ecu_dir: PathBuf,
    /// Interpreter state. `vm.iface` holds the interface (C#: `EdInterfaceClass`).
    vm: Vm,
    /// C#: `_sgbdFs != null`.
    sgbd_open: bool,
    /// C#: `_closeSgbdFs`.
    close_sgbd: bool,
    /// C#: `_jobInfos`, in file order: main file first, then the uses files.
    jobs: Vec<JobInfo>,
    /// C#: `SgbdFileName`, e.g. "jbbf87.prg".
    sgbd_file_name: String,
    /// C#: `_sgbdFileResolveLast`.
    sgbd_file_resolve_last: String,
    /// C#: `_groupName`.
    group_name: String,
    /// C#: `_familyName`.
    family_name: String,
    /// C#: `_groupMappingDict`.
    group_mapping: HashMap<String, VariantInfo>,
    /// C#: `_resultSets`: result set 0 is the system result set.
    result_sets: Vec<ResultSet>,
}

impl Ediabas {
    pub fn new(ecu_dir: impl Into<PathBuf>) -> Self {
        let mut ed = Self {
            ecu_dir: PathBuf::new(),
            vm: Vm::new(Arc::new(Sgbd::empty())),
            sgbd_open: false,
            close_sgbd: false,
            jobs: Vec::new(),
            sgbd_file_name: String::new(),
            sgbd_file_resolve_last: String::new(),
            group_name: String::new(),
            family_name: String::new(),
            group_mapping: HashMap::new(),
            result_sets: Vec::new(),
        };
        // C#: defaults set in the `EdiabasNet` constructor.
        let version = format!(
            "{}.{}.{}",
            (EDIABAS_VERSION >> 8) & 0xF,
            (EDIABAS_VERSION >> 4) & 0xF,
            EDIABAS_VERSION & 0xF
        );
        for (k, v) in [
            ("EdiabasVersion", version.as_str()),
            ("Simulation", "0"),
            ("BipDebugLevel", "0"),
            ("ApiTrace", "0"),
            ("IfhTrace", "0"),
            ("TraceBuffering", "0"),
            ("IfhTraceBuffering", "1"),
            ("AppendTrace", "0"),
            ("LockTrace", "0"),
            ("CompatMode", "1"),
            ("UbattHandling", "0"),
            ("IgnitionHandling", "0"),
            ("ClampHandling", "0"),
            ("RetryComm", "1"),
            ("SystemResults", "1"),
            ("TaskPriority", "0"),
            ("EdicApiHandle", "0"),
        ] {
            ed.set_config_property(k, Some(v));
        }
        ed.set_ecu_dir(ecu_dir);
        ed
    }

    pub fn vm(&self) -> &Vm {
        &self.vm
    }

    pub fn vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }

    pub fn ecu_dir(&self) -> &Path {
        &self.ecu_dir
    }

    /// C#: `SetConfigProperty("EcuPath", ..)`. A changed path closes the SGBD
    /// and clears the group mapping.
    pub fn set_ecu_dir(&mut self, dir: impl Into<PathBuf>) {
        let dir = dir.into();
        let changed = !self.ecu_dir.to_string_lossy().eq_ignore_ascii_case(&dir.to_string_lossy());
        let dir_str = dir.to_string_lossy().into_owned();
        self.set_config_raw("EcuPath", Some(&dir_str));
        if changed {
            self.ecu_dir = dir;
            self.close_sgbd = true;
            self.clear_group_mapping();
        }
    }

    /// C#: `ClearGroupMapping`.
    pub fn clear_group_mapping(&mut self) {
        self.sgbd_file_resolve_last.clear();
        self.group_mapping.clear();
    }

    /// C#: `EdInterfaceClass` setter.
    pub fn set_interface(&mut self, iface: Box<dyn Interface>) {
        // C# also sets "IfhVersion" from `InterfaceVerName`; the Rust trait has no such name.
        let name = iface.interface_type().to_string();
        self.vm.iface = Some(iface);
        self.set_config_property("Interface", Some(&name));
    }

    pub fn iface_mut(&mut self) -> Option<&mut (dyn Interface + 'static)> {
        self.vm.iface.as_deref_mut()
    }

    /// C#: `GetConfigProperty`.
    pub fn config_property(&self, key: &str) -> Option<&str> {
        self.vm.config_property(key)
    }

    /// C#: `SetConfigProperty`. `None` or "" removes an existing key.
    /// "EcuPath" goes through `set_ecu_dir`.
    pub fn set_config_property(&mut self, key: &str, value: Option<&str>) {
        if key.eq_ignore_ascii_case("EcuPath") {
            self.set_ecu_dir(value.unwrap_or(""));
            return;
        }
        if key.eq_ignore_ascii_case("Simulation") {
            let new = string_to_value(value.unwrap_or("")) != 0;
            let old = self.config_property("Simulation").is_some_and(|v| string_to_value(v) != 0);
            if new != old {
                self.close_sgbd = true;
            }
        }
        self.set_config_raw(key, value);
    }

    fn set_config_raw(&mut self, key: &str, value: Option<&str>) {
        let key = key.to_uppercase();
        let existing = self.vm.config.keys().find(|k| k.eq_ignore_ascii_case(&key)).cloned();
        match (existing, value) {
            (Some(k), Some(v)) if !v.is_empty() => {
                self.vm.config.remove(&k);
                self.vm.config.insert(key, v.to_string());
            }
            (Some(k), _) => {
                self.vm.config.remove(&k);
            }
            // C# adds the key even for a null value; an absent key reads the same.
            (None, v) => {
                self.vm.config.insert(key, v.unwrap_or("").to_string());
            }
        }
    }

    /// C#: `SgbdFileName`, e.g. "jbbf87.prg".
    pub fn sgbd_file_name(&self) -> &str {
        &self.sgbd_file_name
    }

    /// C#: `SgbdFileName` setter: a changed name closes the open SGBD.
    pub fn set_sgbd_file_name(&mut self, name: &str) {
        if !self.sgbd_file_name.eq_ignore_ascii_case(name) {
            self.close_sgbd = true;
            self.sgbd_file_name = name.to_string();
        }
    }

    /// The loaded variant, e.g. "jbbf87" (SGBD file name without extension).
    pub fn variant_name(&self) -> String {
        file_stem(&self.sgbd_file_name)
    }

    /// C#: `GroupName`: the group file the variant was resolved from, or "".
    pub fn group_name(&self) -> &str {
        &self.group_name
    }

    /// C#: `FamilyName`.
    pub fn family_name(&self) -> &str {
        &self.family_name
    }

    /// C#: `ResultSets` of the last job. Also filled when the job failed.
    pub fn result_sets(&self) -> &[ResultSet] {
        &self.result_sets
    }

    /// C#: `ArgStringStd`: argument for the standard jobs
    /// (INITIALISIERUNG, IDENTIFIKATION, ENDE). Cleared after each `execute_job`.
    pub fn set_arg_string_std(&mut self, args: &str) {
        self.vm.args.binary_std = crate::ENCODING.encode(args).0.into_owned();
    }

    /// C#: `ArgBinaryStd`.
    pub fn set_arg_binary_std(&mut self, args: &[u8]) {
        self.vm.args.binary_std = args.to_vec();
    }

    /// C#: `ResultsRequests` setter.
    pub fn set_results_requests(&mut self, value: &str) {
        self.vm.requested_results =
            value.split(';').filter(|w| !w.is_empty()).map(str::to_uppercase).collect();
    }

    /// C#: `CloseSgbd`: the SGBD is closed (ENDE job run) before the next job.
    pub fn close_sgbd(&mut self) {
        self.close_sgbd = true;
    }

    /// Closes the SGBD now and runs its ENDE job if INITIALISIERUNG ran
    /// (C#: `CloseSgbdFs`, called from `Dispose`).
    pub fn close(&mut self) -> VmResult<()> {
        self.close_sgbd = false;
        self.close_sgbd_fs()
    }

    /// C#: `ResolveSgbdFile`. Accepts "d_motor", "d_motor.grp", "jbbf87" or
    /// a full path. A path with a directory sets the ECU directory first
    /// (the C# callers set "EcuPath" themselves).
    pub fn resolve_sgbd(&mut self, file_name: &str) -> VmResult<()> {
        let path = Path::new(file_name);
        let mut file_name = file_name;
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            self.set_ecu_dir(parent.to_path_buf());
            file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        }
        let base_file_name = file_stem(file_name).to_lowercase();
        if self.sgbd_file_resolve_last == base_file_name {
            // same name specified
            return Ok(());
        }
        match self.resolve_sgbd_inner(file_name, &base_file_name) {
            Ok(()) => {
                self.sgbd_file_resolve_last = base_file_name;
                Ok(())
            }
            Err(e) => {
                self.group_name.clear();
                self.family_name.clear();
                self.sgbd_file_name.clear();
                self.sgbd_file_resolve_last.clear();
                Err(e)
            }
        }
    }

    fn resolve_sgbd_inner(&mut self, file_name: &str, base_file_name: &str) -> VmResult<()> {
        let file_type = self.get_file_type(file_name)?;
        if file_type == 0 {
            // group file
            let key = base_file_name.to_string();
            let mut variant_name = String::new();
            let mut family_name = String::new();
            if let Some(info) = self.group_mapping.get(&key) {
                variant_name = info.variant_name.clone();
                family_name = info.family_name.clone();
            }

            if variant_name.is_empty() {
                self.set_sgbd_file_name(&format!("{base_file_name}{GROUP_FILE_EXT}"));
                let (variant, family) = self.execute_ident_job()?;
                variant_name = variant.to_lowercase();
                if !family.is_empty() {
                    family_name = family.to_lowercase();
                }
                if variant_name.is_empty() {
                    return Err(VmError::File("ResolveSgbdFile: No variant found".into()));
                }
                // C# quirk: stores the file name argument as the family name.
                self.group_mapping.insert(
                    key,
                    VariantInfo { variant_name: variant_name.clone(), family_name: file_name.to_string() },
                );
            }

            self.group_name = base_file_name.to_string();
            self.family_name = family_name;
            self.set_sgbd_file_name(&format!("{variant_name}{PRG_FILE_EXT}"));
        } else {
            self.group_name.clear();
            self.family_name.clear();
            self.set_sgbd_file_name(&format!("{base_file_name}{PRG_FILE_EXT}"));
        }
        Ok(())
    }

    /// C#: `GetFileType`: the header word at 0x10 (0 = group file) of
    /// `<name>.prg`, or of `<name>.grp` if there is no PRG.
    pub fn get_file_type(&self, file_name: &str) -> VmResult<u32> {
        let full = self.ecu_dir.join(file_name);
        let dir = full.parent().map(Path::to_path_buf).unwrap_or_default();
        let base = file_stem(file_name);
        let local = find_file(&dir, &format!("{base}{PRG_FILE_EXT}"))
            .or_else(|| find_file(&dir, &format!("{base}{GROUP_FILE_EXT}")))
            .ok_or_else(|| VmError::File(format!("GetFileType: File not found: {}", full.display())))?;
        let data = std::fs::read(&local)
            .map_err(|e| VmError::File(format!("GetFileType: Unable to read file {}: {e}", local.display())))?;
        match data.get(0x10..0x14) {
            Some(b) => Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            None => Err(VmError::File(format!("GetFileType: Invalid read size: {}", local.display()))),
        }
    }

    /// C#: `ExecuteJob` with `ArgString` and `ResultsRequests` set first.
    /// Returns `ResultSets`: set 0 is the system result set.
    ///
    /// On error the result sets are still built (C# does this in a
    /// `finally`); read them with `result_sets()`.
    pub fn execute_job(&mut self, job_name: &str, args: &str, results_wanted: &str) -> VmResult<Vec<ResultSet>> {
        let bin = crate::ENCODING.encode(args).0.into_owned();
        self.execute_job_bin(job_name, &bin, results_wanted)
    }

    /// Like `execute_job` with a binary argument (C#: `ArgBinary`).
    pub fn execute_job_bin(&mut self, job_name: &str, args: &[u8], results_wanted: &str) -> VmResult<Vec<ResultSet>> {
        self.vm.args.binary = args.to_vec();
        self.set_results_requests(results_wanted);
        self.vm.job_std = false;
        self.vm.job_std_exit = false;
        let r = self.execute_job_private(job_name, false);
        self.vm.args.binary.clear();
        self.vm.args.binary_std.clear();
        self.vm.job_std = false;
        self.vm.job_std_exit = false;
        r.map(|()| self.result_sets.clone())
    }

    /// C#: `IsJobExisting`.
    pub fn is_job_existing(&self, job_name: &str) -> bool {
        self.sgbd_open && self.get_job_info(job_name).is_some()
    }

    /// Job names callable on the loaded SGBD, including inherited ones.
    pub fn job_names(&mut self) -> VmResult<Vec<String>> {
        if self.close_sgbd {
            self.close_sgbd = false;
            self.close_sgbd_fs()?;
        }
        self.open_sgbd_fs()?;
        let mut names: Vec<String> = Vec::new();
        for j in &self.jobs {
            if j.uses.is_some() && is_init_or_exit(&j.name) {
                continue;
            }
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&j.name)) {
                names.push(j.name.clone());
            }
        }
        Ok(names)
    }

    /// C#: `GetJobInfo`: first job with that name; INITIALISIERUNG and
    /// ENDE of uses files are not callable (C#: `ReadAllJobs`).
    fn get_job_info(&self, job_name: &str) -> Option<JobInfo> {
        self.jobs
            .iter()
            .filter(|j| !(j.uses.is_some() && is_init_or_exit(&j.name)))
            .find(|j| j.name.eq_ignore_ascii_case(job_name))
            .cloned()
    }

    /// C#: `OpenSgbdFs` with `ReadAllUses` and `ReadAllJobs`.
    fn open_sgbd_fs(&mut self) -> VmResult<()> {
        if self.sgbd_open {
            return Ok(());
        }
        let path = find_file(&self.ecu_dir, &self.sgbd_file_name).ok_or_else(|| {
            VmError::File(format!(
                "ExecuteJobPrivate: Open SGBD failed: {}",
                self.ecu_dir.join(&self.sgbd_file_name).display()
            ))
        })?;
        let sgbd = Arc::new(Sgbd::open(&path)?);

        let mut jobs: Vec<JobInfo> =
            sgbd.jobs.iter().map(|j| JobInfo { name: j.name.clone(), address: j.address, uses: None }).collect();
        for uses_name in &sgbd.uses {
            let file = format!("{}{PRG_FILE_EXT}", uses_name.to_lowercase());
            // C# logs and skips uses files that are missing or unreadable.
            let Some(uses_path) = find_file(&self.ecu_dir, &file) else { continue };
            let Ok(uses_sgbd) = Sgbd::open(&uses_path) else { continue };
            let uses_sgbd = Arc::new(uses_sgbd);
            jobs.extend(uses_sgbd.jobs.iter().map(|j| JobInfo {
                name: j.name.clone(),
                address: j.address,
                uses: Some(UsesInfo { name: uses_name.clone(), sgbd: uses_sgbd.clone() }),
            }));
        }

        self.vm.set_sgbd(sgbd);
        self.jobs = jobs;
        self.sgbd_open = true;
        self.vm.request_init = true;
        Ok(())
    }

    /// C#: `CloseSgbdFs`.
    fn close_sgbd_fs(&mut self) -> VmResult<()> {
        if self.sgbd_open {
            let r = if !self.vm.request_init { self.execute_exit_job() } else { Ok(()) };
            self.sgbd_open = false;
            self.jobs.clear();
            r?;
        }
        Ok(())
    }

    /// C#: `ExecuteInitJob`.
    fn execute_init_job(&mut self) -> VmResult<()> {
        let job_std_old = self.vm.job_std;
        self.vm.job_std = true;
        let r = self.execute_init_job_inner();
        self.vm.job_std = job_std_old;
        r
    }

    fn execute_init_job_inner(&mut self) -> VmResult<()> {
        if let Err(e) = self.execute_job_private(JOB_NAME_INIT, true) {
            self.close_sgbd_fs()?; // close file to force a reload
            return Err(e);
        }
        if self.result_sets.len() > 1
            && let Some(r) = self.result_sets[1].get("DONE")
            && r.value == ResultValue::Int(1)
        {
            self.vm.request_init = false;
            return Ok(());
        }
        self.close_sgbd_fs()?; // close file to force a reload
        self.vm.set_error(ErrorCode::EDIABAS_SYS_0010)
    }

    /// C#: `ExecuteExitJob`.
    fn execute_exit_job(&mut self) -> VmResult<()> {
        let job_std_old = self.vm.job_std;
        self.vm.job_std = true;
        self.vm.job_std_exit = true;
        let r = if self.is_job_existing(JOB_NAME_EXIT) { self.execute_job_private(JOB_NAME_EXIT, false) } else { Ok(()) };
        self.vm.job_std = job_std_old;
        self.vm.job_std_exit = false;
        r
    }

    /// C#: `ExecuteIdentJob`. Returns (variant, family).
    fn execute_ident_job(&mut self) -> VmResult<(String, String)> {
        let job_std_old = self.vm.job_std;
        self.vm.job_std = true;
        self.vm.results.clear();
        let r = self.execute_job_private(JOB_NAME_IDENT, false);
        self.vm.job_std = job_std_old;
        r?;

        let mut family_name = String::new();
        if self.result_sets.len() > 1 {
            let set = &self.result_sets[1];
            if let Some(ResultData { value: ResultValue::Str(s), .. }) = set.get("FAMILIE") {
                family_name = s.clone();
            }
            if let Some(ResultData { value: ResultValue::Str(s), .. }) = set.get("VARIANTE") {
                return Ok((s.clone(), family_name));
            }
        }
        Ok((String::new(), family_name))
    }

    /// C#: `ExecuteJobPrivate(string jobName, bool recursive)`.
    fn execute_job_private(&mut self, job_name: &str, recursive: bool) -> VmResult<()> {
        if self.close_sgbd {
            self.close_sgbd = false;
            self.close_sgbd_fs()?;
        }
        self.open_sgbd_fs()?;

        let bip_version = self.vm.sgbd.version.bip_version;
        let ediabas_bip_version =
            ((EDIABAS_VERSION >> 8) & 0xF) << 16 | ((EDIABAS_VERSION >> 4) & 0xF) << 8 | (EDIABAS_VERSION & 0xF);
        if bip_version > ediabas_bip_version {
            return self.vm.set_error(ErrorCode::EDIABAS_BIP_0009);
        }

        let Some(job_info) = self.get_job_info(job_name) else {
            // Virtual jobs (_JOBS, _TABLES, ...) are not ported.
            return self.vm.set_error(ErrorCode::EDIABAS_SYS_0008);
        };
        self.execute_job_code(&job_info, recursive)
    }

    /// C#: `ExecuteJobPrivate(Stream fs, JobInfo jobInfo, bool recursive)`.
    /// The per-job state reset and the opcode loop are in `Vm::run`.
    fn execute_job_code(&mut self, job_info: &JobInfo, recursive: bool) -> VmResult<()> {
        if self.vm.request_init && !recursive {
            self.execute_init_job()?;
        }

        self.vm.code = match &job_info.uses {
            Some(u) => u.sgbd.clone(),
            None => self.vm.sgbd.clone(),
        };
        self.result_sets.clear();
        let ecu_file = file_stem(&self.sgbd_file_name);
        self.set_config_raw("BipEcuFile", Some(&ecu_file));

        let r = self.vm.run(job_info.address);

        // C#: the `finally` block: system results are built even on error.
        let mut sets = std::mem::take(&mut self.vm.result_sets);
        let system = self.create_system_result_dict(job_info, sets.len());
        sets.insert(0, system);
        self.result_sets = sets;
        self.vm.code = self.vm.sgbd.clone();
        self.set_config_raw("BipEcuFile", None);
        r
    }

    /// C#: `CreateSystemResultDict`.
    fn create_system_result_dict(&self, job_info: &JobInfo, set_count: usize) -> ResultSet {
        let mut dict = ResultSet::default();
        let s = |name: &str, v: String| ResultData::new(ResultType::S, name, ResultValue::Str(v));
        let i = |name: &str, v: i64| ResultData::new(ResultType::I, name, ResultValue::Int(v));

        let object_name = match &job_info.uses {
            Some(u) => u.name.clone(),
            None => file_stem(&self.sgbd_file_name),
        };
        let sys_results = self.config_property("SystemResults").map(string_to_value).unwrap_or(0);

        dict.0.push(s("VARIANTE", file_stem(&self.sgbd_file_name).to_uppercase()));
        if IS_MIN_VERSION_760 {
            dict.0.push(s("GRUPPE", self.group_name.to_lowercase()));
            dict.0.push(s("FAMILIE", self.family_name.to_lowercase()));
        }
        dict.0.push(s("OBJECT", object_name));
        dict.0.push(s("JOBNAME", job_info.name.clone()));
        dict.0.push(ResultData::new(ResultType::W, "SAETZE", ResultValue::Int(set_count as i64)));
        if sys_results != 0 {
            dict.0.push(s("JOBSTATUS", self.vm.result_job_status.clone()));
            // C# reads these from the interface only in simulation mode; not ported.
            dict.0.push(i("UBATTCURRENT", -1));
            dict.0.push(i("UBATTHISTORY", -1));
            dict.0.push(i("IGNITIONCURRENT", -1));
            dict.0.push(i("IGNITIONHISTORY", -1));
        }
        for r in &self.vm.sys_results.0 {
            if dict.get(&r.name).is_none() {
                dict.0.push(r.clone());
            }
        }
        dict
    }
}

fn is_init_or_exit(name: &str) -> bool {
    name.eq_ignore_ascii_case(JOB_NAME_INIT) || name.eq_ignore_ascii_case(JOB_NAME_EXIT)
}

/// C#: `Path.GetFileNameWithoutExtension`.
fn file_stem(name: &str) -> String {
    Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Finds `name` in `dir`, ignoring ASCII case (ECU files ship as `.prg` or `.PRG`).
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    let exact = dir.join(name);
    if exact.is_file() {
        return Some(exact);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name) && e.path().is_file())
        .map(|e| e.path())
}

/// C#: `EdiabasNet.StringToValue` (hex "0x", binary "0y", decimal up to '.' or ',').
fn string_to_value(number: &str) -> i64 {
    let local = number.trim_end();
    if local.is_empty() {
        return 0;
    }
    let lower = local.to_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        match hex.chars().next() {
            Some(c) if c.is_ascii_digit() || ('a'..='f').contains(&c) => i64::from_str_radix(hex, 16).unwrap_or(0),
            _ => 0,
        }
    } else if let Some(bin) = lower.strip_prefix("0y") {
        i64::from_str_radix(bin, 2).unwrap_or(0)
    } else if lower == "-" || lower == "--" || lower.chars().next().is_some_and(char::is_alphabetic) {
        0
    } else {
        let conv = local.trim_start();
        let conv = match conv.find(['.', ',']) {
            Some(i) => &conv[..i],
            None => conv,
        };
        conv.parse::<i64>().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XOR: u8 = 0xF7;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("best-job-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Instruction: opcode, modes, operands (plain, XOR applied by `sgbd_file`).
    fn ins(op: u8, a0: &[u8], m0: u8, a1: &[u8], m1: u8) -> Vec<u8> {
        let mut v = vec![op, (m0 << 4) | m1];
        v.extend_from_slice(a0);
        v.extend_from_slice(a1);
        v
    }

    fn imm_str(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        b.push(0);
        let mut v = (b.len() as i16).to_le_bytes().to_vec();
        v.extend(b);
        v
    }

    const M_NONE: u8 = 0;
    const M_IMM8: u8 = 5;
    const M_STR: u8 = 8;

    fn eoj() -> Vec<u8> {
        ins(0x1D, &[], M_NONE, &[], M_NONE)
    }
    fn eoj_status(s: &str) -> Vec<u8> {
        ins(0x1D, &imm_str(s), M_STR, &[], M_NONE)
    }
    fn ergb(name: &str, v: u8) -> Vec<u8> {
        ins(0x34, &imm_str(name), M_STR, &[v], M_IMM8)
    }
    fn ergs(name: &str, v: &str) -> Vec<u8> {
        ins(0x39, &imm_str(name), M_STR, &imm_str(v), M_STR)
    }
    fn enewset() -> Vec<u8> {
        ins(0x40, &[], M_NONE, &[], M_NONE)
    }

    /// Builds a minimal SGBD file: header, uses, jobs, code.
    fn sgbd_file(group: bool, uses: &[&str], jobs: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut f = vec![0u8; 0xA0];
        f[..15].copy_from_slice(b"@EDIABAS OBJECT");
        f[0x10..0x14].copy_from_slice(&(if group { 0i32 } else { 1 }).to_le_bytes());
        for off in [0x7C, 0x84, 0x88, 0x90, 0x94] {
            f[off..off + 4].copy_from_slice(&(-1i32).to_le_bytes());
        }
        let x = |b: &[u8]| b.iter().map(|v| v ^ XOR).collect::<Vec<u8>>();

        let uses_off = f.len() as i32;
        f[0x7C..0x80].copy_from_slice(&uses_off.to_le_bytes());
        f.extend((uses.len() as i32).to_le_bytes());
        for u in uses {
            let mut rec = vec![0u8; 0x100];
            rec[..u.len()].copy_from_slice(u.as_bytes());
            f.extend(x(&rec));
        }

        let jobs_off = f.len();
        f[0x88..0x8C].copy_from_slice(&(jobs_off as i32).to_le_bytes());
        f.extend((jobs.len() as i32).to_le_bytes());
        let mut code_off = jobs_off + 4 + jobs.len() * 0x44;
        let mut code = Vec::new();
        for (name, c) in jobs {
            let mut rec = vec![0u8; 0x44];
            rec[..name.len()].copy_from_slice(name.as_bytes());
            rec[0x40..].copy_from_slice(&(code_off as u32).to_le_bytes());
            f.extend(x(&rec));
            code.extend(x(c));
            code_off += c.len();
        }
        f.extend(code);
        f
    }

    fn init_ok() -> (&'static str, Vec<u8>) {
        ("INITIALISIERUNG", [ergb("DONE", 1), eoj()].concat())
    }

    fn val<'a>(set: &'a ResultSet, name: &str) -> &'a ResultValue {
        &set.get(name).unwrap_or_else(|| panic!("missing {name}")).value
    }

    #[test]
    fn string_to_value_formats() {
        assert_eq!(string_to_value("1"), 1);
        assert_eq!(string_to_value("0x1F"), 31);
        assert_eq!(string_to_value("0y101"), 5);
        assert_eq!(string_to_value(" 12.5"), 12);
        assert_eq!(string_to_value("abc"), 0);
        assert_eq!(string_to_value(""), 0);
    }

    #[test]
    fn find_file_ignores_case() {
        let dir = tmp_dir("case");
        std::fs::write(dir.join("JBBF87.PRG"), b"x").unwrap();
        assert!(find_file(&dir, "jbbf87.prg").is_some());
        assert!(find_file(&dir, "jbbf87.grp").is_none());
    }

    #[test]
    fn results_requests_split_and_upper() {
        let mut ed = Ediabas::new(tmp_dir("req"));
        ed.set_results_requests("a;;Status_x");
        assert_eq!(ed.vm().requested_results, vec!["A".to_string(), "STATUS_X".to_string()]);
    }

    #[test]
    fn program_file_runs_init_then_job_with_system_results() {
        let dir = tmp_dir("prg");
        let file = sgbd_file(
            false,
            &[],
            &[init_ok(), ("INFO", [ergs("ECU", "test ecu"), eoj_status("OKAY")].concat())],
        );
        std::fs::write(dir.join("TEST.PRG"), file).unwrap();

        let mut ed = Ediabas::new(&dir);
        ed.resolve_sgbd("test").unwrap();
        assert_eq!(ed.sgbd_file_name(), "test.prg");
        assert_eq!(ed.group_name(), "");
        let sets = ed.execute_job("INFO", "", "").unwrap();
        assert_eq!(sets.len(), 2);
        let sys = &sets[0];
        let names: Vec<&str> = sys.0.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "VARIANTE",
                "GRUPPE",
                "FAMILIE",
                "OBJECT",
                "JOBNAME",
                "SAETZE",
                "JOBSTATUS",
                "UBATTCURRENT",
                "UBATTHISTORY",
                "IGNITIONCURRENT",
                "IGNITIONHISTORY"
            ]
        );
        assert_eq!(val(sys, "VARIANTE"), &ResultValue::Str("TEST".into()));
        assert_eq!(val(sys, "OBJECT"), &ResultValue::Str("test".into()));
        assert_eq!(val(sys, "JOBNAME"), &ResultValue::Str("INFO".into()));
        assert_eq!(sys.get("SAETZE").unwrap().ty, ResultType::W);
        assert_eq!(val(sys, "SAETZE"), &ResultValue::Int(1));
        assert_eq!(val(sys, "JOBSTATUS"), &ResultValue::Str("OKAY".into()));
        assert_eq!(sys.get("UBATTCURRENT").unwrap().ty, ResultType::I);
        assert_eq!(val(sys, "UBATTCURRENT"), &ResultValue::Int(-1));
        assert_eq!(val(&sets[1], "ECU"), &ResultValue::Str("test ecu".into()));
        assert!(!ed.vm().request_init);
        assert_eq!(ed.config_property("BipEcuFile"), None);
    }

    #[test]
    fn init_without_done_raises_sys_0010() {
        let dir = tmp_dir("noinit");
        let file = sgbd_file(false, &[], &[("INITIALISIERUNG", eoj()), ("INFO", eoj())]);
        std::fs::write(dir.join("t.prg"), file).unwrap();
        let mut ed = Ediabas::new(&dir);
        ed.resolve_sgbd("t").unwrap();
        let r = ed.execute_job("INFO", "", "");
        assert!(matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_SYS_0010))), "{r:?}");
        assert!(ed.vm().request_init);
    }

    #[test]
    fn missing_job_raises_sys_0008() {
        let dir = tmp_dir("nojob");
        std::fs::write(dir.join("t.prg"), sgbd_file(false, &[], &[init_ok()])).unwrap();
        let mut ed = Ediabas::new(&dir);
        ed.resolve_sgbd("t").unwrap();
        let r = ed.execute_job("NOPE", "", "");
        assert!(matches!(r, Err(VmError::Raised(ErrorCode::EDIABAS_SYS_0008))), "{r:?}");
    }

    #[test]
    fn uses_jobs_run_from_base_file() {
        let dir = tmp_dir("uses");
        let main = sgbd_file(false, &["BASE"], &[init_ok(), ("OWN", eoj())]);
        let base = sgbd_file(
            false,
            &[],
            &[
                ("INITIALISIERUNG", [ergs("FROM", "base-init"), eoj()].concat()),
                ("OWN", [ergs("FROM", "base"), eoj()].concat()),
                ("BASEJOB", [ergs("FROM", "base"), eoj()].concat()),
            ],
        );
        std::fs::write(dir.join("main.prg"), main).unwrap();
        std::fs::write(dir.join("BASE.PRG"), base).unwrap();

        let mut ed = Ediabas::new(&dir);
        ed.resolve_sgbd("main.prg").unwrap();
        // The base file's INITIALISIERUNG is hidden, the main one runs.
        let names = ed.job_names().unwrap();
        assert_eq!(names, ["INITIALISIERUNG", "OWN", "BASEJOB"]);

        let sets = ed.execute_job("basejob", "", "").unwrap();
        assert_eq!(val(&sets[0], "OBJECT"), &ResultValue::Str("BASE".into()));
        assert_eq!(val(&sets[0], "VARIANTE"), &ResultValue::Str("MAIN".into()));
        assert_eq!(val(&sets[1], "FROM"), &ResultValue::Str("base".into()));

        // Main file jobs shadow base jobs of the same name.
        let sets = ed.execute_job("OWN", "", "").unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(val(&sets[0], "OBJECT"), &ResultValue::Str("main".into()));
    }

    #[test]
    fn group_file_resolves_variant() {
        let dir = tmp_dir("grp");
        let grp = sgbd_file(
            true,
            &[],
            &[init_ok(), ("IDENTIFIKATION", [ergs("VARIANTE", "VAR1"), ergs("FAMILIE", ""), eoj()].concat())],
        );
        let prg = sgbd_file(false, &[], &[init_ok(), ("FS_LESEN", [ergs("A", "1"), enewset(), ergs("A", "2"), eoj()].concat())]);
        std::fs::write(dir.join("D_TEST.GRP"), grp).unwrap();
        std::fs::write(dir.join("var1.prg"), prg).unwrap();

        let mut ed = Ediabas::new(&dir);
        ed.resolve_sgbd("d_test.grp").unwrap();
        assert_eq!(ed.sgbd_file_name(), "var1.prg");
        assert_eq!(ed.variant_name(), "var1");
        assert_eq!(ed.group_name(), "d_test");
        let sets = ed.execute_job("FS_LESEN", "", "").unwrap();
        assert_eq!(sets.len(), 3);
        assert_eq!(val(&sets[0], "GRUPPE"), &ResultValue::Str("d_test".into()));
        assert_eq!(val(&sets[0], "FAMILIE"), &ResultValue::Str("".into()));
        assert_eq!(val(&sets[0], "OBJECT"), &ResultValue::Str("var1".into()));
        assert_eq!(val(&sets[0], "VARIANTE"), &ResultValue::Str("VAR1".into()));
        assert_eq!(val(&sets[0], "SAETZE"), &ResultValue::Int(2));
    }

    #[test]
    fn group_without_variant_fails_and_resets() {
        let dir = tmp_dir("grpfail");
        let grp = sgbd_file(true, &[], &[init_ok(), ("IDENTIFIKATION", eoj())]);
        std::fs::write(dir.join("d_x.grp"), grp).unwrap();
        let mut ed = Ediabas::new(&dir);
        assert!(matches!(ed.resolve_sgbd("d_x"), Err(VmError::File(_))));
        assert_eq!(ed.sgbd_file_name(), "");
        assert_eq!(ed.group_name(), "");
    }

    #[test]
    fn failed_job_still_builds_result_sets() {
        let dir = tmp_dir("fail");
        // Opcode 0xFF is out of range: the job fails after one finished set.
        let bad = [ergs("A", "1"), enewset(), vec![0xFF, 0x00]].concat();
        std::fs::write(dir.join("t.prg"), sgbd_file(false, &[], &[init_ok(), ("BAD", bad)])).unwrap();
        let mut ed = Ediabas::new(&dir);
        ed.resolve_sgbd("t").unwrap();
        assert!(ed.execute_job("BAD", "", "").is_err());
        let sets = ed.result_sets();
        assert_eq!(sets.len(), 2);
        assert_eq!(val(&sets[0], "JOBNAME"), &ResultValue::Str("BAD".into()));
        assert_eq!(val(&sets[0], "SAETZE"), &ResultValue::Int(1));
    }

    #[test]
    fn missing_file_is_an_error() {
        let mut ed = Ediabas::new(tmp_dir("missing"));
        assert!(matches!(ed.resolve_sgbd("nothere"), Err(VmError::File(_))));
    }

    /// Runs INFO on jbbf87 from `BMWA_ECU_DIR`. Needs all opcodes INFO uses.
    #[test]
    #[ignore]
    fn jbbf87_info() {
        let dir = std::env::var("BMWA_ECU_DIR").expect("BMWA_ECU_DIR not set");
        let mut ed = Ediabas::new(dir);
        ed.resolve_sgbd("jbbf87").unwrap();
        let r = ed.execute_job("INFO", "", "");
        for (i, set) in ed.result_sets().iter().enumerate() {
            println!("set {i}");
            for d in &set.0 {
                println!("  {}: {:?}: {:?}", d.name, d.ty, d.value);
            }
        }
        let sets = r.unwrap();
        let sys = &sets[0];
        assert_eq!(val(sys, "VARIANTE"), &ResultValue::Str("JBBF87".into()));
        assert_eq!(val(sys, "JOBNAME"), &ResultValue::Str("INFO".into()));
        assert_eq!(val(sys, "SAETZE"), &ResultValue::Int(1));
        assert_eq!(val(sys, "UBATTCURRENT"), &ResultValue::Int(-1));
        assert!(sys.get("OBJECT").is_some());
        assert!(sys.get("JOBSTATUS").is_some());
    }
}
