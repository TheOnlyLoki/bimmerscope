//! Remembers the ECU files folder between runs in `$XDG_CONFIG_HOME/bmwa/config.toml`
//! (else `$HOME/.config/bmwa/config.toml`). The file holds one line: `ecu_dir = "<path>"`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const KEY: &str = "ecu_dir";

/// Config file below `base`, the config home directory.
fn path_in(base: &Path) -> PathBuf {
    base.join("bmwa").join("config.toml")
}

/// `$XDG_CONFIG_HOME`, else `$HOME/.config`. Empty variables count as unset.
fn config_home() -> Option<PathBuf> {
    let set = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    set("XDG_CONFIG_HOME").or_else(|| set("HOME").map(|home| home.join(".config")))
}

/// Value of the `ecu_dir = "<path>"` line: text between the first and last `"`.
fn parse(text: &str) -> Option<PathBuf> {
    let line = text
        .lines()
        .map(str::trim_start)
        .find(|l| l.starts_with(KEY))?;
    let first = line.find('"')?;
    let last = line.rfind('"')?;
    (last > first).then(|| PathBuf::from(&line[first + 1..last]))
}

fn load_in(base: &Path) -> Option<PathBuf> {
    fs::read_to_string(path_in(base))
        .ok()
        .and_then(|text| parse(&text))
}

fn save_in(base: &Path, dir: &Path) -> io::Result<()> {
    let utf8 = dir.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "ECU folder path is not UTF-8")
    })?;
    if utf8.contains('"') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ECU folder path contains a double quote",
        ));
    }
    let path = path_in(base);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{KEY} = \"{utf8}\"\n"))
}

/// The remembered ECU folder, if the config file has one.
pub fn load_ecu_dir() -> Option<PathBuf> {
    load_in(&config_home()?)
}

/// Remembers `dir` as the ECU folder. Creates the config directory.
pub fn save_ecu_dir(dir: &Path) -> io::Result<()> {
    let base = config_home().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "neither XDG_CONFIG_HOME nor HOME is set",
        )
    })?;
    save_in(&base, dir)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    /// Fresh temp dir per test. Env vars are not touched.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("bmwa-config-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_quoted_value_with_spaces() {
        assert_eq!(
            parse("ecu_dir = \"/data/ecu files\"\n"),
            Some(PathBuf::from("/data/ecu files"))
        );
    }

    #[test]
    fn skips_other_lines_and_rejects_missing_quotes() {
        assert_eq!(
            parse("# comment\nother = \"x\"\necu_dir = \"/ecu\"\n"),
            Some(PathBuf::from("/ecu"))
        );
        assert_eq!(parse("ecu_dir = /no/quotes\n"), None);
        assert_eq!(parse("ecu_dir = \"only one\n"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn save_then_load_round_trip() {
        let base = scratch("round");
        assert_eq!(load_in(&base), None);

        save_in(&base, Path::new("/home/x/ecu")).unwrap();
        assert_eq!(load_in(&base), Some(PathBuf::from("/home/x/ecu")));
        assert_eq!(
            fs::read_to_string(path_in(&base)).unwrap(),
            "ecu_dir = \"/home/x/ecu\"\n"
        );

        save_in(&base, Path::new("/other")).unwrap();
        assert_eq!(load_in(&base), Some(PathBuf::from("/other")));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn refuses_path_with_quote() {
        let base = scratch("quote");
        assert!(save_in(&base, Path::new("/a\"b")).is_err());
        assert!(!path_in(&base).exists());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn config_file_lives_under_bmwa() {
        assert_eq!(
            path_in(Path::new("/cfg")),
            PathBuf::from("/cfg/bmwa/config.toml")
        );
    }
}
