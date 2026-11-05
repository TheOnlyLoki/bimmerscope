//! bmwa: open source BMW diagnostics in the terminal.

mod app;
mod cable;
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
    /// Directory with the ECU description files (*.prg, *.grp).
    #[arg(long, env = "BMWA_ECU_DIR")]
    ecu_dir: PathBuf,
    /// Answer diagnostic requests from an EdiabasLib .sim file instead of a car.
    #[arg(long, env = "BMWA_SIM_FILE")]
    sim: Option<PathBuf>,
}

/// Diagnostic interface for the worker thread. Built there, since interfaces are not `Send`.
fn make_interface(args: &Args) -> Option<Box<dyn best::Interface>> {
    if let Some(path) = &args.sim {
        return SimInterface::open(path).ok().map(|s| Box::new(s) as Box<dyn best::Interface>);
    }
    // TODO(obd): ObdInterface::new(port)
    None
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(sim) = &args.sim {
        std::fs::metadata(sim).with_context(|| format!("sim file {}", sim.display()))?;
    }
    let files = index::list(&args.ecu_dir).with_context(|| format!("reading {}", args.ecu_dir.display()))?;
    let rx = index::spawn(files.iter().map(|f| f.path.clone()).collect());
    let worker = diag::spawn(args.ecu_dir.clone(), args.clone());
    let mut app = app::App::new(args.ecu_dir, files, rx, worker);
    if args.sim.is_some() {
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
