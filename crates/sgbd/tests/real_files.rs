//! Parses every SGBD in `$BMWA_ECU_DIR`. ECU files are BMW property and are
//! not part of the repo, so the test is skipped when the variable is unset.

#[test]
fn parse_all_ecu_files() {
    let Ok(dir) = std::env::var("BMWA_ECU_DIR") else {
        eprintln!("BMWA_ECU_DIR not set, skipping");
        return;
    };
    let mut failures = Vec::new();
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if ext != "prg" && ext != "grp" {
            continue;
        }
        count += 1;
        if let Err(e) = sgbd::Sgbd::open(&path) {
            failures.push(format!("{}: {e}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{} of {count} failed:\n{}", failures.len(), failures.join("\n"));
    eprintln!("parsed {count} files");
}
