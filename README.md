# BMW Analyzer (`bmwa`)

Open source BMW diagnostics for the terminal, written in Rust. It reads
EDIABAS SGBD files (`.prg` / `.grp`) and talks to the car through a K+DCAN
USB cable. The design follows [EdiabasLib](https://github.com/uholeschak/ediabaslib).

Scope for now: read and clear fault memory, live data. No coding, no flashing.

## ECU files

ECU files are BMW property and are not part of this repository. Point `bmwa`
at your own copy:

```bash
cargo run --release -p bmwa -- --ecu-dir /path/to/ecu
```

or set `BMWA_ECU_DIR`.

## Cable setup (Linux)

Install a udev rule that gives the logged-in user access to the FTDI cable
and sets the latency timer to 1 ms (K-line timing needs it, default is 16 ms):

```bash
printf '%s\n' 'SUBSYSTEM=="tty", ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6001", TAG+="uaccess"' 'ACTION=="add", SUBSYSTEM=="usb-serial", DRIVER=="ftdi_sio", ATTR{latency_timer}="1"' | sudo tee /etc/udev/rules.d/70-kdcan.rules && sudo udevadm control --reload
```

Then replug the cable. Pre-2007 cars (for example E87 pre-LCI) use K-line:
set the cable switch to K-line. The Adapter tab (`4`) shows the checks.

## Roadmap

1. SGBD reader and ECU file browser (done)
2. BEST/2 interpreter: run jobs from the PRG bytecode
3. K-line transport (DS2, KWP2000, BMW-FAST) for the K+DCAN cable
4. Fault memory tab: `FS_LESEN`, `FS_LOESCHEN` per ECU
5. Live data tab: polled `STATUS_*` jobs with graphs
6. D-CAN and ENET transports

## Tests

```bash
BMWA_ECU_DIR=/path/to/ecu cargo test --release
```

parses every SGBD in the directory.

## License

GPL-3.0-or-later.
