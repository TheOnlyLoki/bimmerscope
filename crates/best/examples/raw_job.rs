//! `cargo run -p best --example raw_job -- <file.prg> <JOB>`: run one job
//! on the bare VM (no init job, no interface) and print the results.

use std::sync::Arc;

fn main() {
    let mut a = std::env::args().skip(1);
    let (path, job) = (a.next().expect("file"), a.next().expect("job"));
    let mut vm = best::Vm::new(Arc::new(sgbd::Sgbd::open(&path).unwrap()));
    vm.trace = Some(Vec::new());
    let r = vm.execute_job(&job);
    for line in vm.trace.as_ref().unwrap().iter().rev().take(5).rev() {
        println!("trace {line}");
    }
    println!("result: {r:?}");
    for (i, set) in vm.result_sets.iter().enumerate() {
        println!("DATASET: {}", i + 1);
        for d in &set.0 {
            println!("{}: {:?}", d.name, d.value);
        }
    }
}
