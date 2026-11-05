//! Detects USB serial adapters (K+DCAN cables are FTDI FT232R) via sysfs.
//! Linux only for now.

use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const SYSFS_USB_SERIAL: &str = "/sys/bus/usb-serial/devices";
/// `O_NONBLOCK` on Linux: probe the tty without waiting for carrier.
const O_NONBLOCK: i32 = 0o4000;

#[derive(Debug, Clone)]
pub struct Port {
    pub dev: PathBuf,
    pub driver: String,
    pub vid_pid: String,
    pub product: String,
    pub serial: String,
    pub latency_ms: Option<u32>,
    pub accessible: bool,
}

impl Port {
    pub fn is_ftdi(&self) -> bool {
        self.driver == "ftdi_sio"
    }
}

pub fn scan() -> Vec<Port> {
    let Ok(entries) = fs::read_dir(SYSFS_USB_SERIAL) else {
        return Vec::new();
    };
    let mut ports: Vec<Port> = entries.flatten().filter_map(|e| port(&e.path())).collect();
    ports.sort_by(|a, b| a.dev.cmp(&b.dev));
    ports
}

fn port(sys: &Path) -> Option<Port> {
    let name = sys.file_name()?.to_str()?.to_string();
    let real = fs::canonicalize(sys).ok()?;
    // .../<usb-device>/<interface>/ttyUSBn
    let usb = real.parent()?.parent()?;
    let read = |p: &Path| fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default();
    let driver = fs::read_link(sys.join("driver"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let dev = PathBuf::from("/dev").join(&name);
    let accessible = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(&dev)
        .is_ok();
    Some(Port {
        latency_ms: read(&sys.join("latency_timer")).parse().ok(),
        vid_pid: format!("{}:{}", read(&usb.join("idVendor")), read(&usb.join("idProduct"))),
        product: read(&usb.join("product")),
        serial: read(&usb.join("serial")),
        driver,
        dev,
        accessible,
    })
}
