use crate::{Message, Setting, Status};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use crossbeam::channel::Sender;
use indicatif::ProgressBar;
use md5::{Digest, Md5};
use parking_lot::RwLock;
use rayon::prelude::*;
use regex::Regex;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct HashPair {
    file_path: String,
    expected_hash: String,
}

#[derive(Debug, Clone)]
pub enum StatusWrapper {
    Status(Arc<RwLock<Status>>),
    ProgressBar(ProgressBar),
}

impl StatusWrapper {
    fn set_text(&self, filename: String, file_hash: String, expected_hash: String) {
        match self {
            StatusWrapper::Status(status) => {
                let mut status = status.write();
                status.filename = filename;
                status.file_hash = file_hash;
                status.expected_hash = expected_hash;
            }
            StatusWrapper::ProgressBar(_) => {}
        }
    }

    fn inc_correct(&self) {
        match self {
            StatusWrapper::Status(status) => {
                let mut status = status.write();
                status.correct_num += 1;
            }
            StatusWrapper::ProgressBar(progress) => {
                progress.inc(1);
            }
        }
    }

    fn inc_incorrect(&self) {
        match self {
            StatusWrapper::Status(status) => {
                let mut status = status.write();
                status.incorrect_num += 1;
            }
            StatusWrapper::ProgressBar(progress) => {
                progress.inc(1);
            }
        }
    }

    fn inc_error(&self) {
        match self {
            StatusWrapper::Status(status) => {
                let mut status = status.write();
                status.error_num += 1;
            }
            StatusWrapper::ProgressBar(progress) => {
                progress.inc(1);
            }
        }
    }
}

pub fn hash_list_parser(file_path: &Path) -> Result<Vec<HashPair>> {
    let content = std::fs::read_to_string(file_path)?;
    let re = Regex::new(r"(?m)^[0-9a-z]{32}")?;

    let pair: Vec<_> = content
        .lines()
        .filter_map(|s| {
            if !re.is_match(s) {
                return None;
            }

            let (hash, file) = s.split_once(" ")?;
            Some(HashPair {
                file_path: file
                    .strip_prefix(['*', ' '])
                    .expect("file should always be prefixed")
                    .to_string(),
                expected_hash: hash.to_string(),
            })
        })
        .collect();

    if pair.is_empty() {
        return Err(eyre!("Empty hash list"));
    }

    Ok(pair)
}

fn hashing_file(hash_pair: &HashPair, block_size: usize) -> Result<String> {
    let mut file = File::open(&hash_pair.file_path)?;
    let mut buffer = vec![0u8; block_size];

    let mut hasher = Md5::new();
    while let Ok(bytes_read) = file.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn hash_checker(
    hash_pair: HashPair,
    setting: &Setting,
    status: StatusWrapper,
    tx: Sender<Message>,
) {
    let res = hashing_file(&hash_pair, setting.block_size);
    let file_hash = match res {
        Ok(x) => x,
        Err(e) => {
            let _ = tx.send(Message::Error(e));
            status.inc_error();
            return;
        }
    };

    if hash_pair.expected_hash != file_hash {
        let _ = tx.send(Message::Incorrect(hash_pair.file_path.clone()));
        status.inc_incorrect();
    } else {
        status.inc_correct();
    }

    status.set_text(hash_pair.file_path, file_hash, hash_pair.expected_hash);
}

pub fn prepare_hashing(
    mut hash_list: Vec<HashPair>,
    setting: &Setting,
    status: StatusWrapper,
    tx: Sender<Message>,
) {
    let start_time = Instant::now();
    if setting.sort {
        hash_list.sort();
    }

    if setting.parallel {
        hash_list.into_par_iter().for_each(|x| {
            let tx_clone = tx.clone();
            hash_checker(x, setting, status.clone(), tx_clone)
        });
    } else {
        hash_list.into_iter().for_each(|x| {
            let tx_clone = tx.clone();
            hash_checker(x, setting, status.clone(), tx_clone)
        });
    }

    let _ = tx.send(Message::Completed(Instant::now() - start_time));
}
