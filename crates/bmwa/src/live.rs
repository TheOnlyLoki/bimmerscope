//! Live data: which values to read from a module (presets for the main
//! values, every `STATUS_*` job as the expert fallback), the UI-side model of
//! the polled samples, and CSV recording. The polling itself runs in the
//! `diag` worker. Nothing here talks to the car.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use best::ResultValue;
use sgbd::Sgbd;

use crate::diag::{Command, Event};
use crate::modules::{self, Module};

/// Samples kept per value, and so the longest sparkline.
pub const HISTORY: usize = 120;
/// Round times kept for the update rate.
const RATE_WINDOW: usize = 8;

/// One value a preset can show. Several candidates share a label: the first one the
/// module's variant file really offers wins, so one preset works for every engine.
pub struct Value {
    pub label_en: &'static str,
    pub label_de: &'static str,
    pub unit: &'static str,
    pub job: &'static str,
    pub args: &'static str,
    pub result: &'static str,
    /// The job answers what `args` ask for, so its result names are not in the
    /// job description. Only the job has to exist.
    pub dynamic: bool,
}

const fn val(
    label_en: &'static str,
    label_de: &'static str,
    unit: &'static str,
    job: &'static str,
    result: &'static str,
) -> Value {
    Value {
        label_en,
        label_de,
        unit,
        job,
        args: "",
        result,
        dynamic: false,
    }
}

/// Like `val`, for a job that takes `args` and answers with undocumented results.
const fn dynamic(
    label_en: &'static str,
    label_de: &'static str,
    unit: &'static str,
    job: &'static str,
    args: &'static str,
    result: &'static str,
) -> Value {
    Value {
        label_en,
        label_de,
        unit,
        job,
        args,
        result,
        dynamic: true,
    }
}

pub struct Preset {
    /// Group file name, like `Module::id`.
    pub module: &'static str,
    pub title: &'static str,
    pub candidates: &'static [Value],
}

/// Measurement block of the diesel engine ECUs (D60M47A0 and friends), as the
/// Deep OBD sample page asks for it. One telegram answers all values below.
const DIESEL_BLOCK: &str = "NEIN;IUBAT;ITKUM;CTSCD_tClntLin;ITKRS;ILMKG;ILMMG;SLMMG;ITUMG;IPLAD;SPLAD;ITLAL;IPUMG;IPRDR;SPRDR;ITAVO;ITAVP1;IPDIP;IDSLRE;IREAN;EGT_st;ISRBF;ISOED;ISBLS;IFPWG;IVKMH;IAFZG";
const DIESEL_JOB: &str = "STATUS_MESSWERTBLOCK_LESEN";

/// Petrol MEV9 (N46) has STATUS_MESSWERTE, which answers many values in one
/// telegram: listed first because the K-line is slow. The single-value jobs
/// come second, for variants without it.
static ENGINE: &[Value] = &[
    val(
        "Engine speed",
        "Motordrehzahl",
        "rpm",
        "STATUS_MESSWERTE",
        "STAT_NMOT_WERT",
    ),
    val(
        "Engine speed",
        "Motordrehzahl",
        "rpm",
        "STATUS_MOTORDREHZAHL",
        "STAT_MOTORDREHZAHL_WERT",
    ),
    val(
        "Coolant temperature",
        "Kühlmitteltemperatur",
        "°C",
        "STATUS_MESSWERTE",
        "STAT_TMOT_WERT",
    ),
    dynamic(
        "Coolant temperature",
        "Kühlmitteltemperatur",
        "°C",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_KUEHLMITTELTEMPERATUR_WERT",
    ),
    val(
        "Coolant temperature",
        "Kühlmitteltemperatur",
        "°C",
        "STATUS_MOTORTEMPERATUR",
        "STAT_MOTORTEMPERATUR_WERT",
    ),
    val(
        "Oil temperature",
        "Öltemperatur",
        "°C",
        "OELSENSOR_LESEN",
        "OEL_TEMP_WERT",
    ),
    val(
        "Intake air temperature",
        "Ansauglufttemperatur",
        "°C",
        "STATUS_MESSWERTE",
        "STAT_TANS_WERT",
    ),
    dynamic(
        "Intake air temperature",
        "Ansauglufttemperatur",
        "°C",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_LADELUFTTEMPERATUR_WERT",
    ),
    val(
        "Intake air temperature",
        "Ansauglufttemperatur",
        "°C",
        "STATUS_AN_LUFTTEMPERATUR",
        "STAT_AN_LUFTTEMPERATUR_WERT",
    ),
    val(
        "Battery voltage",
        "Batteriespannung",
        "V",
        "STATUS_MESSWERTE",
        "STAT_UB_WERT",
    ),
    dynamic(
        "Battery voltage",
        "Batteriespannung",
        "V",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_UBATT_WERT",
    ),
    val(
        "Battery voltage",
        "Batteriespannung",
        "V",
        "STATUS_UBATT",
        "STAT_UBATT_WERT",
    ),
    val(
        "Vehicle speed",
        "Fahrgeschwindigkeit",
        "km/h",
        "STATUS_MESSWERTE",
        "STAT_VFZG_WERT",
    ),
    dynamic(
        "Vehicle speed",
        "Fahrgeschwindigkeit",
        "km/h",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_GESCHWINDIGKEIT_WERT",
    ),
    val(
        "Throttle opening",
        "Drosselklappe",
        "%",
        "STATUS_MESSWERTE",
        "STAT_WDKBA_WERT",
    ),
    dynamic(
        "Accelerator pedal",
        "Fahrpedal",
        "%",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_FAHRERWUNSCH_PEDAL_WERT",
    ),
    val(
        "Air mass",
        "Luftmasse",
        "kg/h",
        "STATUS_MESSWERTE",
        "STAT_MSHFM_WERT",
    ),
    dynamic(
        "Air mass",
        "Luftmasse",
        "kg/h",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_LUFTMASSE_WERT",
    ),
    val(
        "Air mass",
        "Luftmasse",
        "kg/h",
        "STATUS_LMM_MASSE",
        "STAT_LMM_MASSE_WERT",
    ),
    val(
        "Oxygen sensor voltage",
        "Lambdasondenspannung",
        "V",
        "STATUS_L_SONDE",
        "STAT_L_SONDE_WERT",
    ),
    dynamic(
        "Boost pressure",
        "Ladedruck",
        "hPa",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_LADEDRUCK_WERT",
    ),
    dynamic(
        "Rail pressure",
        "Raildruck",
        "bar",
        DIESEL_JOB,
        DIESEL_BLOCK,
        "STAT_RAILDRUCK_WERT",
    ),
];

static DSC: &[Value] = &[
    val(
        "Wheel front left",
        "Rad vorne links",
        "km/h",
        "STATUS_RADGESCHWINDIGKEIT",
        "STAT_RAD_GESCHW_VL_WERT",
    ),
    val(
        "Wheel front right",
        "Rad vorne rechts",
        "km/h",
        "STATUS_RADGESCHWINDIGKEIT",
        "STAT_RAD_GESCHW_VR_WERT",
    ),
    val(
        "Wheel rear left",
        "Rad hinten links",
        "km/h",
        "STATUS_RADGESCHWINDIGKEIT",
        "STAT_RAD_GESCHW_HL_WERT",
    ),
    val(
        "Wheel rear right",
        "Rad hinten rechts",
        "km/h",
        "STATUS_RADGESCHWINDIGKEIT",
        "STAT_RAD_GESCHW_HR_WERT",
    ),
    val(
        "Steering angle",
        "Lenkwinkel",
        "°",
        "STATUS_ANALOG",
        "STAT_SUMMEN_LENKWINKEL_WERT_AFS",
    ),
    val(
        "Steering angle",
        "Lenkwinkel",
        "°",
        "STATUS_SENSOREN",
        "STAT_LENKWINKEL_WERT",
    ),
    val(
        "Acceleration forward",
        "Längsbeschleunigung",
        "m/s²",
        "STATUS_ANALOG",
        "STAT_LAENGSBESCHLEUNIGUNG_WERT",
    ),
    val(
        "Acceleration forward",
        "Längsbeschleunigung",
        "g",
        "STATUS_SENSOREN",
        "STAT_LAENGSBESCHLEUNIGUNG_WERT",
    ),
    val(
        "Acceleration sideways",
        "Querbeschleunigung",
        "m/s²",
        "STATUS_ANALOG",
        "STAT_QUERBESCHLEUNIGUNG_WERT_1",
    ),
    val(
        "Acceleration sideways",
        "Querbeschleunigung",
        "g",
        "STATUS_SENSOREN",
        "STAT_QUERBESCHLEUNIGUNG_WERT",
    ),
];

static CLIMATE: &[Value] = &[
    val(
        "Inside temperature",
        "Innentemperatur",
        "°C",
        "STATUS_REGLERGROESSEN",
        "STAT_TINNEN_WERT",
    ),
    val(
        "Inside temperature",
        "Innentemperatur",
        "°C",
        "STATUS_IHX_WERTE",
        "STAT_T_INNEN_WERT",
    ),
    val(
        "Outside temperature",
        "Außentemperatur",
        "°C",
        "STATUS_REGLERGROESSEN",
        "STAT_TAUSSEN_WERT",
    ),
    val(
        "Outside temperature",
        "Außentemperatur",
        "°C",
        "STATUS_IHX_WERTE",
        "STAT_T_AUSSEN_WERT",
    ),
    val(
        "Evaporator temperature",
        "Verdampfertemperatur",
        "°C",
        "STATUS_ANALOGEINGAENGE",
        "STAT_TEMP_VERDAMFER_WERT",
    ),
    val(
        "Evaporator temperature",
        "Verdampfertemperatur",
        "°C",
        "STATUS_IHX_WERTE",
        "STAT_T_VERDAMPFER_WERT",
    ),
    val(
        "Heat exchanger temperature",
        "Wärmetauscher",
        "°C",
        "STATUS_REGLERGROESSEN",
        "STAT_WT_RE_WERT",
    ),
    val(
        "Refrigerant pressure",
        "Kältemitteldruck",
        "bar",
        "STATUS_IHX_WERTE",
        "STAT_DRUCK_KAELTEMITTEL_WERT",
    ),
];

pub static PRESETS: &[Preset] = &[
    Preset {
        module: "d_motor",
        title: "Engine",
        candidates: ENGINE,
    },
    Preset {
        module: "d_dsc",
        title: "Brakes and wheels",
        candidates: DSC,
    },
    Preset {
        module: "d_klima",
        title: "Climate",
        candidates: CLIMATE,
    },
];

pub fn preset_for(group: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.module == group)
}

/// Job as documented in the variant file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatJob {
    pub name: String,
    /// Number of documented arguments. Jobs with arguments are not polled blindly.
    pub args: usize,
    pub results: Vec<CatResult>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatResult {
    pub name: String,
    pub ty: String,
    pub comment: String,
}

/// What a module's resolved variant file offers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Catalog {
    pub group: String,
    pub variant: String,
    pub jobs: Vec<CatJob>,
}

impl Catalog {
    pub fn from_sgbd(group: &str, variant: &str, sgbd: &Sgbd) -> Self {
        let jobs = sgbd
            .description
            .jobs
            .iter()
            .map(|j| CatJob {
                name: j.name.clone(),
                args: j.args.len(),
                results: j
                    .results
                    .iter()
                    .map(|r| CatResult {
                        name: r.name.clone(),
                        ty: r.ty.clone(),
                        comment: r.comments.first().cloned().unwrap_or_default(),
                    })
                    .collect(),
            })
            .collect();
        Self {
            group: group.into(),
            variant: variant.into(),
            jobs,
        }
    }

    fn job(&self, name: &str) -> Option<&CatJob> {
        self.jobs.iter().find(|j| j.name.eq_ignore_ascii_case(name))
    }

    /// Whether the job exists and, unless `loose`, documents the result.
    fn offers(&self, job: &str, result: &str, loose: bool) -> bool {
        self.job(job).is_some_and(|j| {
            loose
                || j.results
                    .iter()
                    .any(|r| r.name.eq_ignore_ascii_case(result))
        })
    }

    /// Every number of every `STATUS_*` job that needs no arguments.
    pub fn expert_items(&self) -> Vec<Item> {
        let mut out = Vec::new();
        for job in &self.jobs {
            if !job.name.to_ascii_uppercase().starts_with("STATUS_") || job.args > 0 {
                continue;
            }
            for r in &job.results {
                if !is_numeric_result(r) {
                    continue;
                }
                out.push(Item {
                    label_en: pretty(&r.name),
                    label_de: r.comment.clone(),
                    unit: String::new(),
                    job: job.name.clone(),
                    args: String::new(),
                    result: r.name.clone(),
                });
            }
        }
        out
    }
}

/// A documented result worth graphing: a number type, or a `_WERT` string
/// (some jobs send numbers as text).
fn is_numeric_result(r: &CatResult) -> bool {
    let name = r.name.to_ascii_uppercase();
    if name.starts_with('_') || name == "JOB_STATUS" || name.ends_with("_TEXT") {
        return false;
    }
    let ty = r.ty.to_ascii_lowercase();
    match ty.as_str() {
        "binary" => false,
        "string" | "" => name.ends_with("_WERT"),
        _ => true,
    }
}

/// `STAT_OEL_TEMP_WERT` -> `OEL TEMP`.
fn pretty(result: &str) -> String {
    let s = result.strip_prefix("STAT_").unwrap_or(result);
    let s = s.strip_suffix("_WERT").unwrap_or(s);
    s.replace('_', " ")
}

/// One value being watched: what to ask the car and how to show it.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub label_en: String,
    pub label_de: String,
    /// Empty: the job reports the unit itself (`..._EINH` result).
    pub unit: String,
    pub job: String,
    pub args: String,
    pub result: String,
}

/// Identifies a result across events: job and result name, upper case.
pub type Key = (String, String);

pub fn key(job: &str, result: &str) -> Key {
    (job.to_ascii_uppercase(), result.to_ascii_uppercase())
}

impl Item {
    pub fn key(&self) -> Key {
        key(&self.job, &self.result)
    }

    fn from_value(v: &Value) -> Self {
        Self {
            label_en: v.label_en.into(),
            label_de: v.label_de.into(),
            unit: v.unit.into(),
            job: v.job.into(),
            args: v.args.into(),
            result: v.result.into(),
        }
    }
}

impl Preset {
    /// The values this variant can deliver, one per label, in label order.
    /// Jobs in `skip` (upper case) are not used, so a failing job can be replaced.
    pub fn resolve(&self, cat: &Catalog, skip: &HashSet<String>) -> Vec<Item> {
        let mut labels: Vec<&str> = Vec::new();
        for c in self.candidates {
            if !labels.contains(&c.label_en) {
                labels.push(c.label_en);
            }
        }
        labels
            .into_iter()
            .filter_map(|label| {
                self.candidates
                    .iter()
                    .filter(|c| c.label_en == label)
                    .find(|c| {
                        !skip.contains(&c.job.to_ascii_uppercase())
                            && cat.offers(c.job, c.result, c.dynamic)
                    })
                    .map(Item::from_value)
            })
            .collect()
    }
}

/// Distinct `(job, args)` to run, in order. One run feeds every value of that job.
pub fn group_jobs(items: &[Item]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for i in items {
        if !out
            .iter()
            .any(|(j, a)| j.eq_ignore_ascii_case(&i.job) && *a == i.args)
        {
            out.push((i.job.clone(), i.args.clone()));
        }
    }
    out
}

/// Number in a result: integers, floats, and numbers sent as text.
pub fn as_number(v: &ResultValue) -> Option<f64> {
    match v {
        ResultValue::Int(i) => Some(*i as f64),
        ResultValue::Float(f) => f.is_finite().then_some(*f),
        ResultValue::Str(s) => s
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|f| f.is_finite()),
        ResultValue::Bytes(_) => None,
    }
}

/// Unit text as ECU files write it, in the form people expect.
pub fn plain_unit(raw: &str) -> String {
    let raw = raw
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    match raw.to_ascii_lowercase().as_str() {
        "grad celsius" | "grad c" | "deg c" | "°c" => "°C".into(),
        "1/min" | "u/min" | "rpm" => "rpm".into(),
        "volt" | "v" => "V".into(),
        "grad" | "deg" => "°".into(),
        "grad/s" | "grad/sec" => "°/s".into(),
        "m/s*s" => "m/s²".into(),
        _ => raw.into(),
    }
}

/// Number for the big card text: fewer decimals for bigger numbers.
pub fn format_value(v: f64) -> String {
    let decimals = match v.abs() {
        _ if v.fract() == 0.0 => 0,
        a if a >= 1000.0 => 0,
        a if a >= 10.0 => 1,
        _ => 2,
    };
    format!("{v:.decimals$}")
}

/// How a value stands against its sane range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Ok,
    Warn,
    Bad,
}

/// Sane range of a value, for a bar gauge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub warn_low: Option<f64>,
    pub warn_high: Option<f64>,
}

impl Range {
    /// Bad outside the scale, warn beyond the warning limits, else ok.
    pub fn zone(&self, v: f64) -> Zone {
        if v < self.min || v > self.max {
            Zone::Bad
        } else if self.warn_low.is_some_and(|w| v < w) || self.warn_high.is_some_and(|w| v > w) {
            Zone::Warn
        } else {
            Zone::Ok
        }
    }

    /// Position on the scale, 0..=1.
    pub fn fraction(&self, v: f64) -> f64 {
        fraction(v, self.min, self.max)
    }
}

/// Bar gauge range for a value, by its label.
pub fn range_for(label_en: &str) -> Option<Range> {
    match label_en {
        "Coolant temperature" | "Oil temperature" => Some(Range {
            min: -20.0,
            max: 140.0,
            warn_low: None,
            warn_high: Some(115.0),
        }),
        "Battery voltage" => Some(Range {
            min: 10.0,
            max: 16.0,
            warn_low: Some(11.8),
            warn_high: Some(15.0),
        }),
        _ => None,
    }
}

/// Round dial: scale, colour zones and tick spacing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dial {
    pub min: f64,
    pub max: f64,
    pub yellow_from: f64,
    pub red_from: f64,
    /// Tick spacing. Every tick gets a label unless `label_every` says otherwise.
    pub major: f64,
    /// Label every n-th tick.
    pub label_every: usize,
    /// Label text is the tick value divided by this.
    pub label_div: f64,
}

impl Dial {
    pub fn zone(&self, v: f64) -> Zone {
        if v >= self.red_from {
            Zone::Bad
        } else if v >= self.yellow_from {
            Zone::Warn
        } else {
            Zone::Ok
        }
    }
}

/// Dial for a value, by its label.
pub fn dial_for(label_en: &str) -> Option<Dial> {
    match label_en {
        "Engine speed" => Some(Dial {
            min: 0.0,
            max: 7000.0,
            yellow_from: 5500.0,
            red_from: 6500.0,
            major: 1000.0,
            label_every: 1,
            label_div: 1000.0,
        }),
        "Vehicle speed" => Some(Dial {
            min: 0.0,
            max: 260.0,
            yellow_from: 180.0,
            red_from: 220.0,
            major: 20.0,
            label_every: 2,
            label_div: 1.0,
        }),
        _ => None,
    }
}

/// The dial arc starts at 210 degrees (lower left) and runs clockwise through
/// the top to -30 degrees (lower right), in maths orientation.
pub const ARC_START: f64 = 210.0;
pub const ARC_SWEEP: f64 = 240.0;

fn fraction(v: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        0.0
    } else {
        ((v - min) / (max - min)).clamp(0.0, 1.0)
    }
}

/// Needle angle in degrees for a value, clamped to the arc.
pub fn needle_angle(v: f64, min: f64, max: f64) -> f64 {
    ARC_START - ARC_SWEEP * fraction(v, min, max)
}

/// Last `HISTORY` samples of one value, with their time in seconds since the run began.
#[derive(Debug, Default)]
pub struct History(VecDeque<(f64, f64)>);

impl History {
    pub fn push(&mut self, t: f64, v: f64) {
        if self.0.len() == HISTORY {
            self.0.pop_front();
        }
        self.0.push_back((t, v));
    }

    pub fn latest(&self) -> Option<f64> {
        self.0.back().map(|&(_, v)| v)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Samples as chart points: x is seconds before `now`, so the newest is near 0.
    pub fn points(&self, now: f64) -> Vec<(f64, f64)> {
        self.0.iter().map(|&(t, v)| (t - now, v)).collect()
    }

    /// The newest `width` samples scaled to 0..=100 between their own min and
    /// max, for a sparkline. A flat line sits in the middle.
    pub fn bars(&self, width: usize) -> Vec<u64> {
        let skip = self.0.len().saturating_sub(width);
        let data: Vec<f64> = self.0.iter().skip(skip).map(|&(_, v)| v).collect();
        let min = data.iter().copied().fold(f64::INFINITY, f64::min);
        let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let span = max - min;
        data.iter()
            .map(|v| {
                if span < f64::EPSILON {
                    50
                } else {
                    ((v - min) / span * 100.0).round() as u64
                }
            })
            .collect()
    }
}

/// CSV field, quoted when it holds a comma, quote or newline.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.into()
    }
}

/// Header cells: `time_s`, then `label [unit]` per value. Labels that would
/// repeat get the job name added so every column is unambiguous.
pub fn csv_header(columns: &[(String, String, String)]) -> String {
    let name = |(label, unit, _): &(String, String, String)| {
        if unit.is_empty() {
            label.clone()
        } else {
            format!("{label} [{unit}]")
        }
    };
    let names: Vec<String> = columns.iter().map(name).collect();
    let mut cells = vec!["time_s".to_string()];
    for (n, (_, _, job)) in names.iter().zip(columns) {
        let repeated = names.iter().filter(|o| *o == n).count() > 1;
        let cell = if repeated {
            format!("{n} ({job})")
        } else {
            n.clone()
        };
        cells.push(csv_field(&cell));
    }
    cells.join(",")
}

/// One CSV row. Missing values are empty cells.
pub fn csv_row(time_s: f64, values: &[Option<f64>]) -> String {
    let mut cells = vec![format!("{time_s:.3}")];
    cells.extend(values.iter().map(|v| v.map_or(String::new(), csv_number)));
    cells.join(",")
}

fn csv_number(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `$XDG_STATE_HOME/bmwa` (else `~/.local/state/bmwa`).
pub fn state_dir() -> Option<PathBuf> {
    let set = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = set("XDG_STATE_HOME").or_else(|| set("HOME").map(|h| h.join(".local/state")))?;
    Some(base.join("bmwa"))
}

/// Writes one CSV row per polling round.
pub struct Recorder {
    pub path: PathBuf,
    start: Instant,
    keys: Vec<Key>,
    out: BufWriter<File>,
    pub rows: usize,
}

impl Recorder {
    /// Creates `live-<unix secs>.csv` in `dir` and writes the header.
    /// `columns` is `(label, unit, job)` per item.
    pub fn create(
        dir: &Path,
        items: &[Item],
        columns: &[(String, String, String)],
    ) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = dir.join(format!("live-{secs}.csv"));
        let mut out = BufWriter::new(File::create(&path)?);
        writeln!(out, "{}", csv_header(columns))?;
        out.flush()?;
        Ok(Self {
            path,
            start: Instant::now(),
            keys: items.iter().map(Item::key).collect(),
            out,
            rows: 0,
        })
    }

    fn write_row(&mut self, at: Instant, samples: &HashMap<Key, History>) {
        let values: Vec<Option<f64>> = self
            .keys
            .iter()
            .map(|k| samples.get(k).and_then(History::latest))
            .collect();
        let t = at.saturating_duration_since(self.start).as_secs_f64();
        // Flushed per row so an aborted run keeps what it recorded.
        if writeln!(self.out, "{}", csv_row(t, &values)).is_ok() && self.out.flush().is_ok() {
            self.rows += 1;
        }
    }
}

/// A finished recording.
#[derive(Debug, PartialEq)]
pub struct Recorded {
    pub path: PathBuf,
    pub rows: usize,
}

impl Recorded {
    pub fn message(&self) -> String {
        format!(
            "Recording saved: {} ({} rows)",
            self.path.display(),
            self.rows
        )
    }
}

/// Modules offered in the picker: the ones that answered the scan, else all.
pub fn picker(found: &[crate::diag::Found]) -> Vec<&'static Module> {
    let list: Vec<&Module> = found.iter().filter_map(|f| modules::find(&f.id)).collect();
    if list.is_empty() {
        modules::MODULES.iter().collect()
    } else {
        list
    }
}

/// UI-side model of the Live data tab.
#[derive(Default)]
pub struct LiveState {
    /// Variant descriptions by group, or the reason there is none.
    pub catalogs: HashMap<String, Result<Catalog, String>>,
    /// Groups whose catalog is asked for or loaded.
    pub requested: HashSet<String>,
    /// Expert list per group.
    pub all_items: HashMap<String, Vec<Item>>,
    /// Watched values per group: the preset's, plus what the user added.
    pub watch: HashMap<String, Vec<Item>>,
    /// Upper case jobs that stopped answering, per group.
    failed: HashMap<String, HashSet<String>>,
    /// Run counter. Events of older runs are stale and ignored.
    pub run: u64,
    pub running: bool,
    /// Module of the current or last run.
    pub group: Option<String>,
    pub samples: HashMap<Key, History>,
    /// Units reported by `..._EINH` results.
    units: HashMap<Key, String>,
    /// Last error per job (upper case), cleared by its next good answer.
    pub errors: HashMap<String, String>,
    rounds: VecDeque<Instant>,
    pub recorder: Option<Recorder>,
    pub expert: bool,
    /// Expert list cursor.
    pub list: ratatui::widgets::ListState,
    /// Value shown in the chart, as index into the watched values.
    pub chart: usize,
    /// Chart overlays up to three values instead of one.
    pub overlay: bool,
    /// Start of the current run, the zero of the chart's time axis.
    t0: Option<Instant>,
    /// Time of the newest sample in seconds since `t0`.
    pub last_t: f64,
    /// Module chosen in the picker.
    pub selected: Option<String>,
    pub notice: Option<String>,
    /// A job was dropped: the app has to start the run again with the new set.
    pub restart: bool,
}

impl LiveState {
    pub fn catalog(&self, group: &str) -> Option<&Catalog> {
        self.catalogs.get(group).and_then(|c| c.as_ref().ok())
    }

    pub fn items(&self, group: &str) -> &[Item] {
        self.watch.get(group).map_or(&[], Vec::as_slice)
    }

    pub fn needs_catalog(&self, group: &str) -> bool {
        !self.requested.contains(group)
    }

    /// Unit to show: the preset's, else what the job reported.
    pub fn unit(&self, item: &Item) -> String {
        if !item.unit.is_empty() {
            return item.unit.clone();
        }
        let upper = item.result.to_ascii_uppercase();
        let einh = match upper.strip_suffix("_WERT") {
            Some(base) => format!("{base}_EINH"),
            None => return String::new(),
        };
        self.units
            .get(&(item.job.to_ascii_uppercase(), einh))
            .map(|u| plain_unit(u))
            .unwrap_or_default()
    }

    pub fn latest(&self, item: &Item) -> Option<f64> {
        self.samples.get(&item.key()).and_then(History::latest)
    }

    /// Values the chart draws: the selected one, plus the next two when overlaying.
    pub fn chart_items(&self, group: &str) -> Vec<&Item> {
        let items = self.items(group);
        if items.is_empty() {
            return Vec::new();
        }
        let first = self.chart.min(items.len() - 1);
        let n = if self.overlay { items.len().min(3) } else { 1 };
        (0..n).map(|k| &items[(first + k) % items.len()]).collect()
    }

    /// Rounds per second over the last few rounds.
    pub fn rate(&self) -> Option<f64> {
        let (first, last) = (self.rounds.front()?, self.rounds.back()?);
        let secs = last.duration_since(*first).as_secs_f64();
        (self.rounds.len() >= 2 && secs > 0.0).then(|| (self.rounds.len() - 1) as f64 / secs)
    }

    /// Adds the item to the watched values of `group`, or removes it.
    pub fn toggle(&mut self, group: &str, item: &Item) {
        let list = self.watch.entry(group.into()).or_default();
        match list.iter().position(|i| i.key() == item.key()) {
            Some(pos) => {
                list.remove(pos);
            }
            None => list.push(item.clone()),
        }
    }

    pub fn is_watched(&self, group: &str, item: &Item) -> bool {
        self.items(group).iter().any(|i| i.key() == item.key())
    }

    /// Run command for the watched values of `group`, or why there is none.
    pub fn start(&mut self, group: &str) -> Result<Command, String> {
        let items = self.items(group);
        if items.is_empty() {
            return Err("Nothing selected. Press A and pick values with Space.".into());
        }
        let jobs = group_jobs(items);
        self.run += 1;
        self.running = true;
        self.group = Some(group.into());
        self.samples.clear();
        self.errors.clear();
        self.rounds.clear();
        self.t0 = Some(Instant::now());
        self.last_t = 0.0;
        self.notice = None;
        Ok(Command::Live {
            run: self.run,
            group: group.into(),
            jobs,
        })
    }

    /// Ends the run and a recording with it. Returns the file a recording went to.
    pub fn stop(&mut self) -> Option<Recorded> {
        self.running = false;
        self.rounds.clear();
        self.stop_recording()
    }

    pub fn stop_recording(&mut self) -> Option<Recorded> {
        self.recorder.take().map(|r| Recorded {
            path: r.path,
            rows: r.rows,
        })
    }

    /// Starts a CSV recording of the watched values of `group` into `dir`.
    pub fn start_recording(&mut self, dir: &Path, group: &str) -> io::Result<PathBuf> {
        let items = self.items(group).to_vec();
        let columns: Vec<(String, String, String)> = items
            .iter()
            .map(|i| (i.label_en.clone(), self.unit(i), i.job.clone()))
            .collect();
        let rec = Recorder::create(dir, &items, &columns)?;
        let path = rec.path.clone();
        self.recorder = Some(rec);
        Ok(path)
    }

    pub fn apply(&mut self, ev: Event) {
        match ev {
            Event::Catalog { group, catalog } => {
                if let Ok(c) = &catalog {
                    self.all_items.insert(group.clone(), c.expert_items());
                    if let Some(p) = preset_for(&group) {
                        let skip = self.failed.get(&group).cloned().unwrap_or_default();
                        self.watch
                            .entry(group.clone())
                            .or_insert_with(|| p.resolve(c, &skip));
                    } else {
                        self.watch.entry(group.clone()).or_default();
                    }
                }
                self.catalogs.insert(group, catalog);
            }
            Event::LiveValues {
                run,
                values,
                at,
                round_end,
            } => {
                if run != self.run {
                    return;
                }
                let t = self
                    .t0
                    .map_or(0.0, |t0| at.saturating_duration_since(t0).as_secs_f64());
                self.last_t = self.last_t.max(t);
                for (job, result, value) in values {
                    let k = key(&job, &result);
                    self.errors.remove(&k.0);
                    if let ResultValue::Str(s) = &value
                        && k.1.ends_with("_EINH")
                    {
                        self.units.insert(k, s.clone());
                    } else if let Some(x) = as_number(&value) {
                        self.samples.entry(k).or_default().push(t, x);
                    }
                }
                if round_end {
                    if self.rounds.len() == RATE_WINDOW {
                        self.rounds.pop_front();
                    }
                    self.rounds.push_back(at);
                    if let Some(rec) = &mut self.recorder {
                        rec.write_row(at, &self.samples);
                    }
                }
            }
            Event::LiveError {
                run,
                job,
                message,
                skipped,
            } => {
                if run != self.run {
                    return;
                }
                self.errors.insert(job.to_ascii_uppercase(), message);
                if skipped {
                    self.drop_job(&job);
                }
            }
            Event::LiveStopped { run, reason } => {
                if run != self.run {
                    return;
                }
                let saved = self.stop();
                self.notice = reason.or(saved.map(|r| r.message())).or(self.notice.take());
            }
            _ => {}
        }
    }

    /// A job gave up: use another candidate for its values, where the preset has one.
    fn drop_job(&mut self, job: &str) {
        let Some(group) = self.group.clone() else {
            return;
        };
        let upper = job.to_ascii_uppercase();
        self.failed
            .entry(group.clone())
            .or_default()
            .insert(upper.clone());
        let replacement = match (preset_for(&group), self.catalog(&group)) {
            (Some(p), Some(c)) => p.resolve(c, &self.failed[&group]),
            _ => Vec::new(),
        };
        let list = self.watch.entry(group).or_default();
        list.retain(|i| i.job.to_ascii_uppercase() != upper);
        for item in replacement {
            if !list.iter().any(|i| i.label_en == item.label_en) {
                list.push(item);
            }
        }
        self.restart = true;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn cat(jobs: &[(&str, &[&str])]) -> Catalog {
        Catalog {
            group: "d_motor".into(),
            variant: "fake".into(),
            jobs: jobs
                .iter()
                .map(|(name, results)| CatJob {
                    name: (*name).into(),
                    args: 0,
                    results: results
                        .iter()
                        .map(|r| CatResult {
                            name: (*r).into(),
                            ty: "real".into(),
                            comment: String::new(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn engine() -> &'static Preset {
        preset_for("d_motor").unwrap()
    }

    fn pair(items: &[Item]) -> Vec<(&str, &str)> {
        items
            .iter()
            .map(|i| (i.label_en.as_str(), i.job.as_str()))
            .collect()
    }

    #[test]
    fn combined_job_wins_over_single_value_jobs() {
        let c = cat(&[
            (
                "STATUS_MESSWERTE",
                &["STAT_NMOT_WERT", "STAT_TMOT_WERT", "STAT_UB_WERT"],
            ),
            ("STATUS_MOTORDREHZAHL", &["STAT_MOTORDREHZAHL_WERT"]),
            ("STATUS_UBATT", &["STAT_UBATT_WERT"]),
        ]);
        let items = engine().resolve(&c, &HashSet::new());
        assert_eq!(
            pair(&items),
            [
                ("Engine speed", "STATUS_MESSWERTE"),
                ("Coolant temperature", "STATUS_MESSWERTE"),
                ("Battery voltage", "STATUS_MESSWERTE"),
            ]
        );
        assert_eq!(group_jobs(&items), [("STATUS_MESSWERTE".into(), "".into())]);
    }

    #[test]
    fn falls_back_to_single_jobs_and_checks_result_names() {
        // STATUS_MESSWERTE exists but does not document STAT_NMOT_WERT.
        let c = cat(&[
            ("STATUS_MESSWERTE", &["STAT_TMOT_WERT"]),
            ("status_motordrehzahl", &["stat_motordrehzahl_wert"]),
            ("STATUS_LMM_MASSE", &["STAT_WRONG_WERT"]),
        ]);
        let items = engine().resolve(&c, &HashSet::new());
        assert_eq!(
            pair(&items),
            [
                ("Engine speed", "STATUS_MOTORDREHZAHL"),
                ("Coolant temperature", "STATUS_MESSWERTE"),
            ]
        );
    }

    #[test]
    fn dynamic_candidates_only_need_the_job() {
        let c = cat(&[(DIESEL_JOB, &["JOB_STATUS"])]);
        let items = engine().resolve(&c, &HashSet::new());
        let labels: Vec<&str> = items.iter().map(|i| i.label_en.as_str()).collect();
        assert!(labels.contains(&"Coolant temperature"));
        assert!(labels.contains(&"Battery voltage"));
        assert!(labels.contains(&"Rail pressure"));
        assert!(!labels.contains(&"Engine speed"));
        // Same job and args for all of them: one telegram.
        assert_eq!(
            group_jobs(&items),
            [(DIESEL_JOB.into(), DIESEL_BLOCK.into())]
        );
    }

    #[test]
    fn skipped_job_is_replaced_by_next_candidate() {
        let c = cat(&[
            ("STATUS_MESSWERTE", &["STAT_NMOT_WERT"]),
            ("STATUS_MOTORDREHZAHL", &["STAT_MOTORDREHZAHL_WERT"]),
        ]);
        let skip: HashSet<String> = ["STATUS_MESSWERTE".to_string()].into();
        let items = engine().resolve(&c, &skip);
        assert_eq!(pair(&items), [("Engine speed", "STATUS_MOTORDREHZAHL")]);
        assert_eq!(items[0].unit, "rpm");
    }

    #[test]
    fn no_matching_job_means_no_values() {
        let c = cat(&[("STATUS_NOTHING", &["STAT_X_WERT"])]);
        assert!(engine().resolve(&c, &HashSet::new()).is_empty());
    }

    #[test]
    fn expert_list_takes_numbers_of_argless_status_jobs() {
        let mut c = cat(&[
            (
                "STATUS_A",
                &["JOB_STATUS", "STAT_X_WERT", "STAT_X_EINH", "_TEL_ANTWORT"],
            ),
            ("IDENT", &["STAT_ID_WERT"]),
        ]);
        c.jobs[0].results[2].ty = "string".into();
        c.jobs[0].results[3].ty = "binary".into();
        c.jobs.push(CatJob {
            name: "STATUS_WITH_ARGS".into(),
            args: 1,
            results: c.jobs[0].results.clone(),
        });
        let items = c.expert_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].result, "STAT_X_WERT");
        assert_eq!(items[0].label_en, "X");
    }

    #[test]
    fn history_keeps_last_samples_only() {
        let mut h = History::default();
        for i in 0..(HISTORY + 30) {
            h.push(i as f64, i as f64);
        }
        assert_eq!(h.len(), HISTORY);
        assert_eq!(h.latest(), Some((HISTORY + 29) as f64));
        // Oldest kept sample is 30.
        let bars = h.bars(1000);
        assert_eq!(bars.len(), HISTORY);
        assert_eq!(bars[0], 0);
        assert_eq!(bars[HISTORY - 1], 100);
        assert_eq!(h.bars(10).len(), 10);
    }

    #[test]
    fn flat_history_sits_in_the_middle() {
        let mut h = History::default();
        h.push(0.0, 5.0);
        h.push(0.0, 5.0);
        assert_eq!(h.bars(10), [50, 50]);
        assert!(History::default().bars(10).is_empty());
    }

    #[test]
    fn numbers_come_from_ints_floats_and_text() {
        assert_eq!(as_number(&ResultValue::Int(3)), Some(3.0));
        assert_eq!(as_number(&ResultValue::Float(1.5)), Some(1.5));
        assert_eq!(as_number(&ResultValue::Str(" 25,8 ".into())), Some(25.8));
        assert_eq!(as_number(&ResultValue::Str("OKAY".into())), None);
        assert_eq!(as_number(&ResultValue::Float(f64::NAN)), None);
        assert_eq!(as_number(&ResultValue::Bytes(vec![1])), None);
    }

    #[test]
    fn value_text_and_units() {
        assert_eq!(format_value(745.0), "745");
        assert_eq!(format_value(745.26), "745.3");
        assert_eq!(format_value(1520.4), "1520");
        assert_eq!(format_value(13.84), "13.8");
        assert_eq!(format_value(-0.0271), "-0.03");
        assert_eq!(plain_unit("Grad Celsius"), "°C");
        assert_eq!(plain_unit("[Grad/sec]"), "°/s");
        assert_eq!(plain_unit("1/min"), "rpm");
        assert_eq!(plain_unit("kg/h"), "kg/h");
    }

    fn col(label: &str, unit: &str, job: &str) -> (String, String, String) {
        (label.into(), unit.into(), job.into())
    }

    #[test]
    fn csv_header_and_rows() {
        let cols = [col("Engine speed", "rpm", "J1"), col("Flag", "", "J1")];
        assert_eq!(csv_header(&cols), "time_s,Engine speed [rpm],Flag");
        assert_eq!(csv_row(0.5, &[Some(750.0), None]), "0.500,750,");
        assert_eq!(
            csv_row(12.3456, &[Some(13.8412), Some(-0.5)]),
            "12.346,13.8412,-0.5"
        );
    }

    #[test]
    fn csv_quotes_commas_and_disambiguates_repeats() {
        let cols = [
            col("Temp, left", "°C", "J1"),
            col("Speed", "km/h", "J1"),
            col("Speed", "km/h", "J2"),
        ];
        assert_eq!(
            csv_header(&cols),
            "time_s,\"Temp, left [°C]\",Speed [km/h] (J1),Speed [km/h] (J2)"
        );
    }

    #[test]
    fn items_are_grouped_by_job_and_args() {
        let item = |job: &str, args: &str, result: &str| Item {
            label_en: result.into(),
            label_de: String::new(),
            unit: String::new(),
            job: job.into(),
            args: args.into(),
            result: result.into(),
        };
        let items = [
            item("A", "", "X"),
            item("B", "", "Y"),
            item("a", "", "Z"),
            item("A", "1", "W"),
        ];
        assert_eq!(
            group_jobs(&items),
            [
                ("A".into(), "".into()),
                ("B".into(), "".into()),
                ("A".into(), "1".into())
            ]
        );
    }

    fn state_with_engine() -> LiveState {
        let mut s = LiveState::default();
        let c = cat(&[
            ("STATUS_MESSWERTE", &["STAT_NMOT_WERT", "STAT_UB_WERT"]),
            ("STATUS_MOTORDREHZAHL", &["STAT_MOTORDREHZAHL_WERT"]),
        ]);
        s.apply(Event::Catalog {
            group: "d_motor".into(),
            catalog: Ok(c),
        });
        s
    }

    fn values(run: u64, round_end: bool, v: &[(&str, &str, ResultValue)]) -> Event {
        Event::LiveValues {
            run,
            values: v
                .iter()
                .map(|(j, r, x)| ((*j).into(), (*r).into(), x.clone()))
                .collect(),
            at: Instant::now(),
            round_end,
        }
    }

    #[test]
    fn catalog_event_fills_watch_list_once() {
        let mut s = state_with_engine();
        assert_eq!(s.items("d_motor").len(), 2);
        s.toggle("d_motor", &s.items("d_motor")[0].clone());
        assert_eq!(s.items("d_motor").len(), 1);
        // A second catalog does not undo the user's choice.
        let again = s.catalogs["d_motor"].clone();
        s.apply(Event::Catalog {
            group: "d_motor".into(),
            catalog: again,
        });
        assert_eq!(s.items("d_motor").len(), 1);
        assert!(s.all_items.contains_key("d_motor"));
    }

    #[test]
    fn run_collects_samples_and_ignores_stale_runs() {
        let mut s = state_with_engine();
        let Ok(Command::Live { run, jobs, .. }) = s.start("d_motor") else {
            panic!("no run");
        };
        assert_eq!(jobs, [("STATUS_MESSWERTE".to_string(), String::new())]);
        assert!(s.running);

        s.apply(values(
            run,
            false,
            &[
                (
                    "STATUS_MESSWERTE",
                    "STAT_NMOT_WERT",
                    ResultValue::Float(750.0),
                ),
                (
                    "STATUS_MESSWERTE",
                    "STAT_NMOT_EINH",
                    ResultValue::Str("1/min".into()),
                ),
                (
                    "STATUS_MESSWERTE",
                    "JOB_STATUS",
                    ResultValue::Str("OKAY".into()),
                ),
            ],
        ));
        s.apply(values(
            run - 1,
            true,
            &[(
                "STATUS_MESSWERTE",
                "STAT_NMOT_WERT",
                ResultValue::Float(9999.0),
            )],
        ));
        let rpm = s.items("d_motor")[0].clone();
        assert_eq!(s.latest(&rpm), Some(750.0));
        assert_eq!(s.rate(), None);

        s.apply(values(run, true, &[]));
        std::thread::sleep(Duration::from_millis(5));
        s.apply(values(run, true, &[]));
        assert!(s.rate().is_some_and(|r| r > 0.0));

        // Stale stop does not end the current run.
        s.apply(Event::LiveStopped {
            run: run - 1,
            reason: None,
        });
        assert!(s.running);
        s.apply(Event::LiveStopped {
            run,
            reason: Some("gone".into()),
        });
        assert!(!s.running);
        assert_eq!(s.notice.as_deref(), Some("gone"));
    }

    #[test]
    fn reported_unit_is_used_when_preset_has_none() {
        let mut s = LiveState {
            run: 1,
            ..Default::default()
        };
        let item = Item {
            label_en: "X".into(),
            label_de: String::new(),
            unit: String::new(),
            job: "STATUS_A".into(),
            args: String::new(),
            result: "STAT_X_WERT".into(),
        };
        assert_eq!(s.unit(&item), "");
        s.apply(values(
            1,
            false,
            &[(
                "STATUS_A",
                "STAT_X_EINH",
                ResultValue::Str("Grad Celsius".into()),
            )],
        ));
        assert_eq!(s.unit(&item), "°C");
    }

    #[test]
    fn skipped_job_switches_to_another_source_and_restarts() {
        let mut s = state_with_engine();
        let Ok(Command::Live { run, .. }) = s.start("d_motor") else {
            panic!("no run");
        };
        s.apply(Event::LiveError {
            run,
            job: "STATUS_MESSWERTE".into(),
            message: "no answer".into(),
            skipped: true,
        });
        assert!(s.restart);
        assert_eq!(
            s.errors.get("STATUS_MESSWERTE").map(String::as_str),
            Some("no answer")
        );
        // The rpm value moves to the single-value job, battery has no other source.
        assert_eq!(
            pair(s.items("d_motor")),
            [("Engine speed", "STATUS_MOTORDREHZAHL")]
        );
    }

    #[test]
    fn needle_angle_runs_along_the_arc_and_is_clamped() {
        assert_eq!(needle_angle(0.0, 0.0, 7000.0), 210.0);
        assert_eq!(needle_angle(3500.0, 0.0, 7000.0), 90.0);
        assert_eq!(needle_angle(7000.0, 0.0, 7000.0), -30.0);
        assert_eq!(needle_angle(-50.0, 0.0, 7000.0), 210.0);
        assert_eq!(needle_angle(9000.0, 0.0, 7000.0), -30.0);
        assert_eq!(needle_angle(130.0, 0.0, 260.0), 90.0);
        // A broken scale does not move the needle.
        assert_eq!(needle_angle(5.0, 10.0, 10.0), 210.0);
        assert_eq!(needle_angle(5.0, 10.0, 0.0), 210.0);
    }

    #[test]
    fn dial_zones_follow_the_limits() {
        let rpm = dial_for("Engine speed").unwrap();
        assert_eq!(rpm.zone(800.0), Zone::Ok);
        assert_eq!(rpm.zone(5499.0), Zone::Ok);
        assert_eq!(rpm.zone(5500.0), Zone::Warn);
        assert_eq!(rpm.zone(6499.0), Zone::Warn);
        assert_eq!(rpm.zone(6500.0), Zone::Bad);
        assert_eq!(rpm.zone(7000.0), Zone::Bad);
        let kmh = dial_for("Vehicle speed").unwrap();
        assert_eq!((kmh.min, kmh.max), (0.0, 260.0));
        assert_eq!(kmh.zone(120.0), Zone::Ok);
        assert!(dial_for("Air mass").is_none());
    }

    #[test]
    fn gauge_ranges_warn_and_clamp() {
        let coolant = range_for("Coolant temperature").unwrap();
        assert_eq!(coolant.zone(90.0), Zone::Ok);
        assert_eq!(coolant.zone(115.0), Zone::Ok);
        assert_eq!(coolant.zone(116.0), Zone::Warn);
        assert_eq!(coolant.zone(150.0), Zone::Bad);
        assert_eq!(coolant.fraction(60.0), 0.5);
        assert_eq!(coolant.fraction(500.0), 1.0);
        assert_eq!(coolant.fraction(-99.0), 0.0);
        let battery = range_for("Battery voltage").unwrap();
        assert_eq!(battery.zone(14.2), Zone::Ok);
        assert_eq!(battery.zone(11.5), Zone::Warn);
        assert_eq!(battery.zone(15.5), Zone::Warn);
        assert_eq!(battery.zone(9.0), Zone::Bad);
        assert!(range_for("Vehicle speed").is_none());
    }

    #[test]
    fn chart_shows_selected_value_or_three_overlaid() {
        let mut s = state_with_engine();
        // Only two values in this fake: overlay cannot exceed that.
        assert_eq!(s.chart_items("d_motor").len(), 1);
        s.overlay = true;
        assert_eq!(s.chart_items("d_motor").len(), 2);
        s.chart = 1;
        let labels: Vec<&str> = s
            .chart_items("d_motor")
            .iter()
            .map(|i| i.label_en.as_str())
            .collect();
        assert_eq!(labels, ["Battery voltage", "Engine speed"]);
        // A selection past the end falls back to the last value.
        s.chart = 40;
        s.overlay = false;
        assert_eq!(s.chart_items("d_motor")[0].label_en, "Battery voltage");
        assert!(s.chart_items("d_cas").is_empty());
    }

    #[test]
    fn samples_carry_time_since_run_start() {
        let mut h = History::default();
        h.push(1.0, 10.0);
        h.push(3.5, 20.0);
        assert_eq!(h.points(3.5), [(-2.5, 10.0), (0.0, 20.0)]);
    }

    #[test]
    fn start_without_values_explains_itself() {
        let mut s = LiveState::default();
        assert!(s.start("d_cas").is_err());
        assert!(!s.running);
    }

    #[test]
    fn recording_writes_header_and_one_row_per_round() {
        let dir = std::env::temp_dir().join(format!("bmwa-live-{}", std::process::id()));
        let mut s = state_with_engine();
        let Ok(Command::Live { run, .. }) = s.start("d_motor") else {
            panic!("no run");
        };
        let path = s.start_recording(&dir, "d_motor").unwrap();
        s.apply(values(
            run,
            false,
            &[(
                "STATUS_MESSWERTE",
                "STAT_NMOT_WERT",
                ResultValue::Float(750.0),
            )],
        ));
        s.apply(values(run, true, &[]));
        s.apply(values(
            run,
            true,
            &[("STATUS_MESSWERTE", "STAT_UB_WERT", ResultValue::Float(13.8))],
        ));
        assert_eq!(
            s.stop(),
            Some(Recorded {
                path: path.clone(),
                rows: 2
            })
        );
        assert!(s.recorder.is_none());

        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "time_s,Engine speed [rpm],Battery voltage [V]");
        assert_eq!(lines.len(), 3);
        assert!(lines[1].ends_with(",750,"));
        assert!(lines[2].ends_with(",750,13.8"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
