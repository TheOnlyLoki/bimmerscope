//! `cargo run --release -p best --example info_all -- <ecu-dir>`: runs the
//! offline INFO job of every PRG on the bare VM and reports failures.

use std::collections::BTreeMap;
use std::sync::Arc;

fn main() {
    let dir = std::env::args().nth(1).expect("ecu dir");
    let (mut ok, mut failures) = (0, BTreeMap::<String, Vec<String>>::new());
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("prg")) {
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let mut vm = best::Vm::new(Arc::new(sgbd::Sgbd::open(&p).unwrap()));
        match vm.execute_job("INFO") {
            Ok(()) if !vm.result_sets.is_empty() => ok += 1,
            Ok(()) => failures.entry("no results".into()).or_default().push(name),
            Err(err) => failures.entry(err.to_string()).or_default().push(name),
        }
    }
    println!("ok: {ok}");
    for (err, files) in &failures {
        println!(
            "{:5}  {err}  e.g. {}",
            files.len(),
            files[..files.len().min(3)].join(", ")
        );
    }
}
