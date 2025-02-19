mod ui;
mod hash;

use std::time::Duration;
use crate::ui::App;
use color_eyre::{Report, Result};

// TODO: Add error messages
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

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut term = ratatui::init();
    let app_result = App::default().run(&mut term);
    ratatui::restore();
    app_result
}
