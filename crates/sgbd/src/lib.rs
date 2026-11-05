//! Reader for EDIABAS SGBD files (`.prg` / `.grp`).
//!
//! An SGBD is a compiled BEST/2 object. Apart from a few header words,
//! every structure is XOR-obfuscated with `0xF7`. Strings use Windows-1252.
//! Layout follows EdiabasLib (`EdiabasNet.cs`, GPL-3.0).

mod desc;

use std::path::Path;

pub use desc::{Description, JobDoc, Param};

const MAGIC: &[u8] = b"@EDIABAS OBJECT";
const XOR_KEY: u8 = 0xF7;

const OFF_KIND: usize = 0x10;
const OFF_ARRAY_SIZE: usize = 0x18;
const OFF_USES: usize = 0x7C;
const OFF_TABLES: usize = 0x84;
const OFF_JOBS: usize = 0x88;
const OFF_DESC: usize = 0x90;
const OFF_VERSION: usize = 0x94;

const JOB_RECORD: usize = 0x44;
const TABLE_RECORD: usize = 0x50;
const USES_RECORD: usize = 0x100;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not an EDIABAS object")]
    BadMagic,
    #[error("unexpected end of file at offset {0:#x}")]
    Eof(usize),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Group file: identifies the ECU variant and points to the matching PRG.
    Group,
    /// Program file: jobs for one ECU variant.
    Program,
}

#[derive(Debug, Clone, Default)]
pub struct Version {
    pub bip_version: u32,
    pub revision: String,
    pub author: String,
    pub from: String,
    pub package: i32,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub name: String,
    /// Offset of the job's bytecode in the file.
    pub address: u32,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// A parsed SGBD file. Keeps the raw bytes for the BEST/2 interpreter.
#[derive(Debug, Clone)]
pub struct Sgbd {
    pub kind: Kind,
    /// Size of each BEST/2 string register (header 0x18, 0 means 1024).
    pub array_size: u32,
    pub version: Version,
    pub uses: Vec<String>,
    pub jobs: Vec<Job>,
    pub tables: Vec<Table>,
    pub description: Description,
    data: Vec<u8>,
}

impl Sgbd {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::parse(std::fs::read(path)?)
    }

    pub fn parse(data: Vec<u8>) -> Result<Self> {
        let r = Reader(&data);
        r.check_magic()?;
        Ok(Self {
            kind: r.kind()?,
            array_size: match r.plain_i32(OFF_ARRAY_SIZE)? as u32 {
                0 => 1024,
                n => n,
            },
            version: r.version()?,
            uses: r.uses()?,
            jobs: r.jobs()?,
            tables: r.tables()?,
            description: r.description()?,
            data,
        })
    }

    /// An SGBD with no jobs, tables or code; for tests.
    pub fn empty() -> Self {
        Self {
            kind: Kind::Program,
            array_size: 1024,
            version: Version::default(),
            uses: Vec::new(),
            jobs: Vec::new(),
            tables: Vec::new(),
            description: Description::default(),
            data: Vec::new(),
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }
}

/// Reads only the kind and description block. Much cheaper than a full
/// parse; used for indexing a whole ECU directory.
pub fn read_summary(path: impl AsRef<Path>) -> Result<(Kind, Description)> {
    let data = std::fs::read(path)?;
    let r = Reader(&data);
    r.check_magic()?;
    Ok((r.kind()?, r.description()?))
}

fn decode(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    encoding_rs::WINDOWS_1252.decode_without_bom_handling(&bytes[..end]).0.into_owned()
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn raw(&self, off: usize, len: usize) -> Result<&[u8]> {
        self.0.get(off..off + len).ok_or(Error::Eof(off))
    }

    fn plain_i32(&self, off: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(self.raw(off, 4)?.try_into().unwrap()))
    }

    fn dec(&self, off: usize, len: usize) -> Result<Vec<u8>> {
        Ok(self.raw(off, len)?.iter().map(|b| b ^ XOR_KEY).collect())
    }

    fn dec_u32(&self, off: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(self.dec(off, 4)?.try_into().unwrap()))
    }

    /// Header pointer; negative means the section is absent.
    fn section(&self, header_off: usize) -> Result<Option<usize>> {
        let off = self.plain_i32(header_off)?;
        Ok((off >= 0).then_some(off as usize))
    }

    fn check_magic(&self) -> Result<()> {
        match self.0.starts_with(MAGIC) {
            true => Ok(()),
            false => Err(Error::BadMagic),
        }
    }

    fn kind(&self) -> Result<Kind> {
        Ok(match self.plain_i32(OFF_KIND)? {
            0 => Kind::Group,
            _ => Kind::Program,
        })
    }

    fn version(&self) -> Result<Version> {
        let Some(off) = self.section(OFF_VERSION)? else {
            return Ok(Version::default());
        };
        let b = self.dec(off, 0x6C)?;
        let i16_at = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
        Ok(Version {
            bip_version: u32::from(b[2]) << 16 | u32::from(b[1]) << 8 | u32::from(b[0]),
            revision: format!("{}.{}", i16_at(0x06), i16_at(0x04)),
            author: decode(&b[0x08..0x48]),
            from: decode(&b[0x48..0x68]),
            package: i32::from_le_bytes(b[0x68..0x6C].try_into().unwrap()),
        })
    }

    fn uses(&self) -> Result<Vec<String>> {
        let Some(off) = self.section(OFF_USES)? else {
            return Ok(Vec::new());
        };
        let count = self.plain_i32(off)?.max(0) as usize;
        (0..count)
            .map(|i| Ok(decode(&self.dec(off + 4 + i * USES_RECORD, USES_RECORD)?)))
            .collect()
    }

    fn jobs(&self) -> Result<Vec<Job>> {
        let Some(off) = self.section(OFF_JOBS)? else {
            return Ok(Vec::new());
        };
        let count = self.plain_i32(off)?.max(0) as usize;
        (0..count)
            .map(|i| {
                let rec = self.dec(off + 4 + i * JOB_RECORD, JOB_RECORD)?;
                Ok(Job {
                    name: decode(&rec[..0x40]),
                    address: u32::from_le_bytes(rec[0x40..0x44].try_into().unwrap()),
                })
            })
            .collect()
    }

    fn tables(&self) -> Result<Vec<Table>> {
        let Some(off) = self.section(OFF_TABLES)? else {
            return Ok(Vec::new());
        };
        // Unlike the job and uses counts, the table count is obfuscated.
        let count = self.dec_u32(off)? as usize;
        (0..count).map(|i| self.table(off + 4 + i * TABLE_RECORD)).collect()
    }

    fn table(&self, off: usize) -> Result<Table> {
        let rec = self.dec(off, TABLE_RECORD)?;
        let u32_at = |o: usize| u32::from_le_bytes(rec[o..o + 4].try_into().unwrap()) as usize;
        let (mut pos, cols, rows) = (u32_at(0x40), u32_at(0x48), u32_at(0x4C));

        // Row 0 holds the column names. Cells are NUL-terminated strings.
        let mut cell = || -> Result<String> {
            let len = self.0[pos.min(self.0.len())..]
                .iter()
                .position(|&b| b == XOR_KEY)
                .ok_or(Error::Eof(pos))?;
            let s = decode(&self.dec(pos, len)?);
            pos += len + 1;
            Ok(s)
        };
        let mut grid = (0..=rows).map(|_| (0..cols).map(|_| cell()).collect::<Result<Vec<_>>>());
        let columns = grid.next().transpose()?.unwrap_or_default();
        Ok(Table {
            name: decode(&rec[..0x40]),
            columns,
            rows: grid.collect::<Result<_>>()?,
        })
    }

    fn description(&self) -> Result<Description> {
        let Some(off) = self.section(OFF_DESC)? else {
            return Ok(Description::default());
        };
        let len = self.plain_i32(off)?.max(0) as usize;
        let text = encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(&self.dec(off + 4, len)?)
            .0
            .into_owned();
        Ok(Description::parse(&text))
    }
}
