use crate::{Message, Setting, Status};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use itertools::Itertools;
use md5::{Digest, Md5};
use parking_lot::RwLock;
use regex::Regex;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use crossbeam::channel::Sender;
use rayon::prelude::*;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct HashPair {
    file_path: String,
    expected_hash: String,
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

            let (hash, file) = s.split(" *").collect_tuple()?;
            Some(HashPair {
                file_path: file.to_string(),
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

fn hash_checker(hash_pair: &HashPair, setting: &Setting, status: Arc<RwLock<Status>>, tx: Sender<Message>) {
    let res = hashing_file(hash_pair, setting.block_size);
    let file_hash = match res {
        Ok(x) => x,
        Err(e) => {
            let _ = tx.send(Message::Error(e));
            let mut status = status.write();
            status.error_num += 1;
            return;
        }
    };

    let mut is_incorrect = false;
    {
        let mut status = status.write();
        if hash_pair.expected_hash != file_hash {
            status.incorrect_num += 1;
            is_incorrect = true;
        } else {
            status.correct_num += 1;
        }
        status.expected_hash = hash_pair.expected_hash.clone();
        status.file_hash = file_hash;
        status.filename = hash_pair.file_path.clone();
    }
    if is_incorrect {
        let _ = tx.send(Message::Incorrect(hash_pair.file_path.clone()));
    }
}

pub fn prepare_hashing(mut hash_list: Vec<HashPair>, setting: &Setting, status: Arc<RwLock<Status>>, tx: Sender<Message>) {
    let start_time = Instant::now();
    if setting.sort {
        hash_list.sort();
    }

    if setting.parallel {
        hash_list.into_par_iter().for_each(|x| {
            let tx_clone = tx.clone();
           hash_checker(&x, setting, Arc::clone(&status), tx_clone)
        });
    } else {
        hash_list.into_iter().for_each(|x| {
            let tx_clone = tx.clone();
            hash_checker(&x, setting, Arc::clone(&status), tx_clone)
        });
    }

    let _ = tx.send(Message::Completed(Instant::now() - start_time));
}
