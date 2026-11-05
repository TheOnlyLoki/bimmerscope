//! bmwa: open source BMW diagnostics in the terminal.

mod app;
mod cable;
mod index;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Directory with the ECU description files (*.prg, *.grp).
    #[arg(long, env = "BMWA_ECU_DIR")]
    ecu_dir: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let files = index::list(&args.ecu_dir).with_context(|| format!("reading {}", args.ecu_dir.display()))?;
    let rx = index::spawn(files.iter().map(|f| f.path.clone()).collect());
    let mut app = app::App::new(args.ecu_dir, files, rx);

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
