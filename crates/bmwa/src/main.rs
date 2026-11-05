//! bmwa (bimmerscope): open source BMW diagnostics in the terminal.

mod app;
mod cable;
mod config;
mod diag;
mod index;
mod modules;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use best::sim::SimInterface;
use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

#[derive(Parser, Clone)]
#[command(version, about)]
struct Args {
    /// Directory with the ECU description files (*.prg, *.grp). Remembered for next runs.
    #[arg(long, env = "BMWA_ECU_DIR")]
    ecu_dir: Option<PathBuf>,
    /// Answer diagnostic requests from an EdiabasLib .sim file instead of a car.
    #[arg(long, env = "BMWA_SIM_FILE")]
    sim: Option<PathBuf>,
    /// Serial port of the K+DCAN cable. Default: first FTDI cable found.
    #[arg(long, env = "BMWA_PORT")]
    port: Option<String>,
}

/// Diagnostic interface for the worker thread. Built there, since interfaces are not `Send`.
fn make_interface(args: &Args) -> Option<Box<dyn best::Interface>> {
    if let Some(path) = &args.sim {
        return SimInterface::open(path).ok().map(|s| Box::new(s) as Box<dyn best::Interface>);
    }
    let port = args.port.clone().or_else(|| {
        cable::scan().into_iter().find(|p| p.is_ftdi()).map(|p| p.dev.display().to_string())
    })?;
    Some(Box::new(obd::ObdInterface::new(&port)))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(sim) = &args.sim {
        std::fs::metadata(sim).with_context(|| format!("sim file {}", sim.display()))?;
    }
    let (ecu_dir, given) = match args.ecu_dir.clone() {
        Some(dir) => (dir, true),
        None => match config::load_ecu_dir() {
            Some(dir) => (dir, false),
            None => {
                eprintln!("No ECU files folder set. Run once with: bmwa --ecu-dir /path/to/ecu");
                std::process::exit(2);
            }
        },
    };
    let files = index::list(&ecu_dir).with_context(|| format!("reading {}", ecu_dir.display()))?;
    if given && let Err(e) = config::save_ecu_dir(&ecu_dir) {
        eprintln!("warning: could not remember ECU folder: {e}");
    }
    let rx = index::spawn(files.iter().map(|f| f.path.clone()).collect());
    let worker = diag::spawn(ecu_dir.clone(), args.clone());
    let mut app = app::App::new(ecu_dir, files, rx, worker);
    if args.sim.is_some() || args.port.is_some() {
        app.tab = app::Tab::Faults;
    }

    ratatui::run(|terminal| -> anyhow::Result<()> {
        while !app.quit {
            app.tick();
            terminal.draw(|f| ui::draw(f, &mut app))?;
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                app.on_key(key);
            }
        }
        Ok(())
    })
}
