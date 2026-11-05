//! Lists the ECU directory and reads each file's ECU comment in the
//! background, so the browser can search by description.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use sgbd::Kind;

#[derive(Debug, Clone)]
pub struct EcuFile {
    pub name: String,
    pub path: PathBuf,
    pub kind: Option<Kind>,
    pub comment: String,
    /// Lowercase name + comment, for filtering.
    pub haystack: String,
}

pub struct Update {
    pub index: usize,
    pub kind: Option<Kind>,
    pub comment: String,
}

pub fn list(dir: &Path) -> std::io::Result<Vec<EcuFile>> {
    let mut files: Vec<EcuFile> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            ext.eq_ignore_ascii_case("prg") || ext.eq_ignore_ascii_case("grp")
        })
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            EcuFile { haystack: name.to_lowercase(), name, path, kind: None, comment: String::new() }
        })
        .collect();
    files.sort_by_key(|f| f.name.to_lowercase());
    Ok(files)
}

pub fn spawn(paths: Vec<PathBuf>) -> Receiver<Update> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for (index, path) in paths.iter().enumerate() {
            let (kind, comment) = match sgbd::read_summary(path) {
                Ok((kind, desc)) => (Some(kind), desc.ecu_comment()),
                Err(e) => (None, format!("<{e}>")),
            };
            if tx.send(Update { index, kind, comment }).is_err() {
                return;
            }
        }
    });
    rx
}
