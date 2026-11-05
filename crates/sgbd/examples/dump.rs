//! `cargo run -p sgbd --example dump -- <file.prg>`: print an SGBD summary.

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump <file>");
    let s = sgbd::Sgbd::open(&path).unwrap();
    println!(
        "kind={:?} ecu={:?} rev={} uses={:?}",
        s.kind,
        s.description.get("ECU"),
        s.version.revision,
        s.uses
    );
    println!("comment: {}", s.description.ecu_comment());
    println!(
        "{} jobs: {:?}",
        s.jobs.len(),
        s.jobs.iter().take(8).map(|j| &j.name).collect::<Vec<_>>()
    );
    for t in &s.tables {
        println!("table {} {:?} rows={}", t.name, t.columns, t.rows.len());
    }
    if let Some(t) = s.table("FORTTEXTE") {
        for r in t.rows.iter().take(4) {
            println!("  {r:?}");
        }
    }
}
