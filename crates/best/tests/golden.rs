//! Golden tests: runs each job captured by `tools/oracle/capture.sh` and
//! diffs the result sets against EdiabasLib's output.
//!
//! Needs BMW files, so it only runs when these are set:
//! `BMWA_ECU_DIR`, `BMWA_SIM_FILE` and `BMWA_GOLDEN_DIR` (capture output).

mod common;

use best::Ediabas;
use best::sim::SimInterface;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Golden output from the first "DATASET:" line on.
fn expected_sets(text: &str) -> &str {
    text.find("DATASET:").map_or("", |i| &text[i..])
}

#[test]
fn golden_jobs_match_ediabaslib() {
    let (Some(ecu), Some(sim), Some(golden)) = (env("BMWA_ECU_DIR"), env("BMWA_SIM_FILE"), env("BMWA_GOLDEN_DIR")) else {
        eprintln!("BMWA_ECU_DIR / BMWA_SIM_FILE / BMWA_GOLDEN_DIR not set, skipping");
        return;
    };
    let mut paths: Vec<_> = std::fs::read_dir(&golden).unwrap().flatten().map(|e| e.path()).collect();
    paths.sort();
    let mut failed = Vec::new();
    for path in paths {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if path.extension().is_none_or(|e| e != "txt") {
            continue;
        }
        let Some((sgbd, job)) = stem.split_once("__") else { continue };
        let args = std::fs::read_to_string(path.with_extension("args")).unwrap_or_default();
        let text = std::fs::read_to_string(&path).unwrap();

        let mut ediabas = Ediabas::new(&ecu);
        ediabas.set_interface(Box::new(SimInterface::open(&sim).unwrap()));
        let run = ediabas.resolve_sgbd(sgbd).and_then(|_| ediabas.execute_job(job, args.trim(), ""));
        let actual = common::format_result_sets(ediabas.result_sets());
        let expected = expected_sets(&text);

        // EdiabasLib reports IFH-0026 when no SGBD-specific sim file exists
        // and the interface sim file is not loaded yet; the job still runs.
        // That is a quirk of the oracle setup, not of the job.
        let oracle_error = text.lines().any(|l| l.starts_with("Error occured") && !l.contains("IFH-0026"));
        if actual.trim_end() == expected.trim_end() && oracle_error == run.is_err() {
            eprintln!("ok   {sgbd} {job}");
            continue;
        }
        eprintln!("FAIL {sgbd} {job}: rust error {:?}, oracle error {oracle_error}", run.err());
        for (i, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
            if a != e {
                eprintln!("  line {i}: rust   {a:?}\n          oracle {e:?}");
            }
        }
        let (na, ne) = (actual.lines().count(), expected.lines().count());
        if na != ne {
            eprintln!("  line count: rust {na}, oracle {ne}");
        }
        failed.push(format!("{sgbd} {job}"));
    }
    assert!(failed.is_empty(), "golden mismatches: {failed:?}");
}
