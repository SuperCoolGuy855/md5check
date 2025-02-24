use crate::hash::{hash_list_parser, prepare_hashing, StatusWrapper};
use crate::{Message, Setting};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use crossbeam::channel;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

pub fn cli_mode(file_path: PathBuf, setting: Setting) -> Result<()> {
    if !file_path.is_file() {
        return Err(eyre!("Path is not file: {file_path:?}"));
    }

    if file_path.is_absolute() {
        std::env::set_current_dir(
            file_path
                .parent()
                .expect("If path is absolute and is a file, then a parent exists"),
        )?;
    }

    let hash_list = hash_list_parser(&file_path)?;

    let style = ProgressStyle::with_template(
        r"[{elapsed_precise}] [ETA:{eta_precise}] {wide_bar} {pos}/{len} {msg}",
    )
    .expect("How can this fail?");
    let progress = ProgressBar::new(hash_list.len() as u64).with_style(style);
    let status = StatusWrapper::ProgressBar(progress.clone());

    let (tx, rx) = channel::unbounded();

    std::thread::spawn(move || prepare_hashing(hash_list, &setting, status, tx));

    loop {
        let res = rx.recv();
        let mess = match res {
            Ok(mess) => mess,
            Err(e) => return Err(e.into()),
        };

        match mess {
            Message::Incorrect(s) => progress.set_message(format!("Incorrect: {s}")),
            Message::Error(e) => progress.set_message(format!("Error: {e}")),
            Message::Completed(_) => break,
            Message::Empty => {}
        }
    }

    progress.finish();

    Ok(())
}
