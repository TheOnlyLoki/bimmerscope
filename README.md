# bimmerscope

Open source diagnostics for BMW cars, in your terminal. Written in Rust.

Plug in a K+DCAN USB cable, press `S`, and see which control units in your
car have stored faults, in plain words. No Windows VM, no INPA, no
cracked software.

```
▐███▌ BIMMERSCOPE  1 ECU files  2 Fault memory  3 Live data  4 Adapter       ● /dev/ttyUSB0
╭──────────────────╮╭──────────────────────────────────────────────────────────────────╮
│   [S] Scan car   ││Scan done: 14 found, 1 not answering.                             │
╰──────────────────╯╰──────────────────────────────────────────────────────────────────╯
╭ Modules (14) ──────────────────────────────╮╭ Park distance ─────────────────────────╮
│Engine                   OK  Motor          ││╭─────────────────────────────── 0xE205 ╮│
│Brakes DSC               OK  Bremsregelung  │││CAN-High, physical bus error          ││
│Park distance            1 fault  Parkhilfe │││Not present now, but stored           ││
│Navigation               OK  Navigation     ││╰──────────────────────────────────────╯│
 S  scan   ↑↓  module   PgUp/PgDn  faults   C  clear faults   1-4  tabs   q  quit
```

> **Status: early.** Works against recorded car data (EdiabasLib `.sim` files)
> and matches EdiabasLib output job for job. First tests on a real car (E87,
> K-line) are in progress.

## Features

- **Fault memory**: scan all modules, read faults with their text, clear them
  (with confirmation and a re-read to check the clear worked).
- **ECU file browser**: search all your SGBD files, see their jobs, results
  and tables.
- **Adapter check**: finds the cable, checks permissions and the USB latency
  timer, tells you what to fix.
- **Made for non-experts**: modules have plain names ("Engine", "Park
  distance"), not `D_MOTOR` or `JBBF87`.

Planned: live data with graphs, D-CAN (2007+ cars), ENET (F/G series).
Coding and flashing are out of scope.

## How it works

BMW's own tool chain (EDIABAS) describes every control unit in an SGBD file
(`.prg` / `.grp`). These files contain bytecode for a small virtual machine
(BEST/2) that builds the request telegrams and decodes the answers.

bimmerscope has its own implementation of that stack:

| Crate | What it does |
|---|---|
| `crates/sgbd` | Reads SGBD files: jobs, tables, documentation |
| `crates/best` | BEST/2 interpreter: runs jobs from the bytecode, all 184 opcodes |
| `crates/obd` | Serial transport for K+DCAN cables: DS2, KWP2000, BMW-FAST, 5-baud and fast init |
| `crates/bmwa` | The terminal app |

The design follows [EdiabasLib](https://github.com/uholeschak/ediabaslib) by
Ulrich Holeschak, which made this project possible. Golden tests run the same
jobs through EdiabasLib and bimmerscope and compare the results.

## You need

- A **K+DCAN USB cable** (FTDI based). Cars before about 03/2007 use the K-line:
  set the switch on the cable to K. Later cars use D-CAN.
- **ECU files** (`.prg` / `.grp`). They are BMW property and are **not**
  included. You need your own copy.
- Linux. Other platforms are planned.
- Rust (stable, edition 2024).

## Quick start

```bash
git clone https://github.com/TheOnlyLoki/bimmerscope
cd bimmerscope
cargo run --release -p bmwa -- --ecu-dir /path/to/ecu
```

The ECU folder is remembered in `~/.config/bmwa/config.toml`, so next time
`cargo run --release -p bmwa` is enough. The binary is called `bmwa`.

Options:

| Option | Env | Meaning |
|---|---|---|
| `--ecu-dir` | `BMWA_ECU_DIR` | Folder with your `.prg` / `.grp` files |
| `--port` | `BMWA_PORT` | Serial port of the cable. Default: first FTDI cable found |
| `--sim` | `BMWA_SIM_FILE` | Use an EdiabasLib `.sim` file instead of a car |

### Cable setup (Linux)

Give the logged-in user access to the cable and set the USB latency timer to
1 ms (K-line timing needs it; the default is 16 ms):

```bash
printf '%s\n' 'SUBSYSTEM=="tty", ATTRS{idVendor}=="0403", ATTRS{idProduct}=="6001", TAG+="uaccess"' 'ACTION=="add", SUBSYSTEM=="usb-serial", DRIVER=="ftdi_sio", ATTR{latency_timer}="1"' | sudo tee /etc/udev/rules.d/70-kdcan.rules && sudo udevadm control --reload
```

Then unplug and replug the cable. The Adapter tab (`4`) shows whether
everything is ready.

### Run a single job

For debugging, run one job and see the raw telegrams:

```bash
cargo run -p obd --example car_job -- /path/to/ecu /dev/ttyUSB0 d_motor IDENT
```

## Tests

```bash
cargo test --workspace
```

Extra tests run when you point them at your files:

```bash
# parse every SGBD file
BMWA_ECU_DIR=/path/to/ecu cargo test --release -p sgbd
# compare against EdiabasLib output (see tools/oracle)
BMWA_ECU_DIR=... BMWA_SIM_FILE=... BMWA_GOLDEN_DIR=$PWD/target/golden cargo test --release -p best --test golden
```

`tools/oracle` builds EdiabasLib's test tool in the .NET 8 Docker image and
records reference output.

## Safety

Reading faults is harmless. Clearing faults only deletes the stored entries;
it does not repair anything. Do not run diagnostics while driving. Use at your
own risk.

## License

GPL-3.0-or-later, like EdiabasLib. Not affiliated with or endorsed by BMW AG.
BMW is a trademark of BMW AG.
