//! `cargo run -p best --example job -- <ecu-dir> <sim-file> <sgbd> <job> [args]`:
//! runs a job through the job runner with the simulation interface and
//! prints the result sets, or the last instructions on error.

use best::Ediabas;
use best::sim::SimInterface;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut ediabas = Ediabas::new(&a[0]);
    ediabas.set_interface(Box::new(SimInterface::open(&a[1]).unwrap()));
    ediabas.vm_mut().trace = Some(Vec::new());
    let r = ediabas
        .resolve_sgbd(&a[2])
        .and_then(|_| ediabas.execute_job(&a[3], a.get(4).map_or("", |s| s), ""));
    if let Ok(path) = std::env::var("BMWA_TRACE_FILE") {
        std::fs::write(path, ediabas.vm().trace.as_ref().unwrap().join("\n")).unwrap();
    }
    if let Err(e) = &r {
        println!("error: {e}");
        for l in ediabas
            .vm()
            .trace
            .as_ref()
            .unwrap()
            .iter()
            .rev()
            .take(12)
            .rev()
        {
            println!("  {l}");
        }
    }
    for (i, set) in ediabas.result_sets().iter().enumerate() {
        println!("DATASET: {i}");
        for d in &set.0 {
            println!("  {}: {:?}", d.name, d.value);
        }
    }
}
