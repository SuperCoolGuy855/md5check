use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};
use std::env::set_current_dir;
use std::io::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use clap::Parser;
use colored::Colorize;
use md5::{Digest, Md5};
use rayon::prelude::*;
use regex::Regex;

trait Hex {
    fn to_hex(&self) -> String;
}

trait ConsistentMD5 {
    fn con_update(&mut self, bytes: &[u8]);
    fn con_finalize(&mut self) -> Vec<u8>;
}

impl ConsistentMD5 for Md5 {
    fn con_update(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }

    fn con_finalize(&mut self) -> Vec<u8> {
        self.finalize_reset().to_vec()
    }
}

#[derive(Debug, Parser, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Use multithreading to speed up hashing")]
    multithreading: bool,

    #[arg(short, long, default_value_t = false, help = "Enable a timer")]
    time: bool,

    #[arg(short, long, default_value_t = 1024, help = "Block size in KiB")]
    block_size: usize,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Sort file by decreasing size"
    )]
    sort: bool,

    #[arg(short, long, help = "Set current working directory")]
    cwd: Option<String>,

    #[arg(help = "File contains hash list")]
    filename: String,
}

impl Hex for Vec<u8> {
    fn to_hex(&self) -> String {
        let mut out = String::new();
        for v in self {
            out = format!("{out}{v:02x?}");
        }
        out
    }
}

static ARGS_CELL: OnceLock<Args> = OnceLock::new();

fn md5_hash_list_parser(filename: &str) -> Result<(HashMap<String, PathBuf>, Vec<usize>), Error> {
    let file = BufReader::new(File::open(filename)?);
    let mut unreadable = vec![];
    let mut hash_list = HashMap::new();
    let re = Regex::new(r"^[0-9a-z]{32}").unwrap();

    for (i, line) in file.lines().enumerate() {
        let line = if let Ok(line) = line {
            line
        } else {
            unreadable.push(i);
            continue;
        };

        let res = line.split_once(" *");
        if let Some((hash, filepath)) = res {
            if !re.is_match(hash) {
                unreadable.push(i);
                continue;
            }

            let path = PathBuf::from(filepath);

            hash_list.entry(hash.to_string()).or_insert(path);
        } else {
            unreadable.push(i);
            continue;
        }
    }

    Ok((hash_list, unreadable))
}

fn file_checker(block_size: usize, hash: &str, filepath: &Path) -> Result<bool, Error> {
    let mut file = BufReader::new(match File::open(filepath) {
        Ok(f) => f,
        Err(_) => return Ok(false),
    });

    let args = ARGS_CELL.get().unwrap();

    let mut hasher = Md5::new();
    let mut buffer = vec![0; block_size];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.con_update(&buffer[..bytes_read]);
    }
    let result = hasher.con_finalize().to_hex();

    Ok(result == hash)
}

fn main() {
    let before = Instant::now();
    let args = Args::parse();
    // let args = Args {
    //     multithreading: true,
    //     time: true,
    //     block_size: 1024,
    //     sort: false,
    //     cwd: Some("F:/Games/No Man's Sky/_Redist".to_string()),
    //     filename: "fitgirl.md5".to_string(),
    // };

    ARGS_CELL.set(args).unwrap();
    let args = ARGS_CELL.get().unwrap();

    if let Some(path) = args.cwd.as_ref() {
        set_current_dir(path).unwrap();
    }

    let (hash_list, unreadable) = md5_hash_list_parser(&args.filename).unwrap();

    let hash_list_len = hash_list.len();

    let hash_list: Vec<(String, PathBuf)> = if !args.sort {
        hash_list.into_iter().collect()
    } else {
        let mut temp: Vec<(String, PathBuf, u64)> = hash_list
            .into_iter()
            .map(|(hash, path)| {
                let file = File::open(&path).unwrap();
                let size = file.metadata().unwrap().len();

                (hash, path, size)
            })
            .collect();

        temp.sort_unstable_by_key(|x| x.2);
        temp.into_iter().rev().map(|(a, b, _)| (a, b)).collect()
    };

    fn checker_wrapper(hash: String, path: PathBuf, args: &Args) -> Option<PathBuf> {
        let mut incorrect_file = None;
        let status_string = if file_checker(args.block_size * 1024, &hash, &path).unwrap() {
            "correct".bright_green()
        } else {
            incorrect_file = Some(path.clone());
            "wrong".bright_red()
        };

        println!("{} {}", path.to_string_lossy(), status_string);
        incorrect_file
    }

    let mut incorrect_file = vec![];
    if args.multithreading {
        incorrect_file = hash_list
            .into_par_iter()
            .fold(
                || vec![],
                |mut acc, (hash, path)| {
                    let res = checker_wrapper(hash, path, &args);
                    if let Some(x) = res {
                        acc.push(x);
                    }
                    acc
                },
            )
            .reduce(
                || vec![],
                |mut a, mut b| {
                    a.append(&mut b);
                    a
                },
            );
    } else {
        hash_list.into_iter().for_each(|(hash, path)| {
            let res = checker_wrapper(hash, path, &args);
            if let Some(x) = res {
                incorrect_file.push(x);
            }
        });
    }

    let incorrect_len = incorrect_file.len();
    let correct_len = hash_list_len - incorrect_len;

    println!("Correct: {correct_len}/{hash_list_len}");
    println!("Incorrect: {incorrect_len}/{hash_list_len}");
    for path in incorrect_file {
        println!("{} {}", path.to_string_lossy(), "wrong".bright_red());
    }
    println!("Unreadable lines: {unreadable:?}");
    if args.time {
        let time = Instant::now() - before;
        println!("{:?}", time)
    }
}
