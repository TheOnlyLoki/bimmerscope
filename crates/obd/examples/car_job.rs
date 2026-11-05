//! `cargo run -p obd --example car_job -- <ecu-dir> <port> <sgbd> <job> [args]`:
//! runs a job in the car through a K-line cable (e.g. K+DCAN on
//! `/dev/ttyUSB0`) and prints the result sets and the telegram log.
//!
//! Environment: `BMWA_TRACE_FILE=<path>` writes the BEST/2 trace,
//! `BMWA_TELEGRAM_FILE=<path>` writes the telegram log.

use best::Ediabas;
use obd::ObdInterface;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: car_job <ecu-dir> <port> <sgbd> <job> [args]");
        std::process::exit(2);
    }
    let mut iface = ObdInterface::new(&a[1]);
    iface.set_sgbd_file_name(&a[2]);
    let log = iface.log_handle();

    let mut ediabas = Ediabas::new(&a[0]);
    ediabas.set_interface(Box::new(iface));
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

    // Closes the port (interface drop stops the comm thread).
    let _ = ediabas.close();
    drop(ediabas);

    let log = log.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(path) = std::env::var("BMWA_TELEGRAM_FILE") {
        std::fs::write(path, log.join("\n")).unwrap();
    }
    println!("TELEGRAMS:");
    for l in log.iter() {
        println!("  {l}");
    }
    if r.is_err() {
        std::process::exit(1);
    }
}
