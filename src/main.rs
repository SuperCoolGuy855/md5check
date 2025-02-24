mod cli;
mod hash;
mod ui;

use crate::cli::cli_mode;
use crate::ui::App;
use clap::Parser;
use color_eyre::{Report, Result};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Default, Clone)]
struct Status {
    filename: String,
    file_hash: String,
    expected_hash: String,
    correct_num: usize,
    incorrect_num: usize,
    error_num: usize,
}

#[derive(Debug)]
enum Message {
    Incorrect(String),
    Error(Report),
    Completed(Duration),
    Empty,
}

// TODO: Add core_num setting
#[derive(Debug, Clone, Copy)]
struct Setting {
    parallel: bool,
    sort: bool,
    block_size: usize,
}

impl Default for Setting {
    fn default() -> Self {
        Self {
            parallel: true,
            sort: false,
            block_size: 8192,
        }
    }
}

impl From<Args> for Setting {
    fn from(mut value: Args) -> Self {
        Self {
            parallel: value.parallel,
            sort: value.sort,
            block_size: value.block_size,
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(version, about)]
struct Args {
    #[arg(short, long)]
    parallel: bool,
    #[arg(short, long)]
    sort: bool,
    #[arg(short, long, default_value_t = Setting::default().block_size)]
    block_size: usize,
    #[arg(short, long)]
    file_path: PathBuf,
}

fn main() -> Result<()> {
    match Args::try_parse() {
        Ok(settings) => {
            cli_mode(settings.file_path.clone(), settings.into())
        }
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
            eprintln!("{e}");
            Ok(())
        }
        _ => {
            color_eyre::install()?;
            let mut term = ratatui::init();
            let app_result = App::default().run(&mut term);
            ratatui::restore();
            app_result
        }
    }
}
