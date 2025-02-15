use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::{self, BufRead, BufReader, Read},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

use clap::Parser;
use colored::Colorize;
use md5::{Digest, Md5};
use rayon::prelude::*;

// TODO: Add arguments (block_size)
const BLOCK_SIZE: usize = 50 * 1024 * 1024;

trait Hex {
    fn to_hex(&self) -> String;
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rayon: bool,

    #[arg(short, long)]
    filename: String,

    #[arg(short, long, default_value_t = 4)]
    thread_num: usize,

    #[arg(short, long, default_value_t = false)]
    benchmark: bool,
}

#[derive(Debug)]
enum Error {
    IOError(io::Error),
    BufReadWrong,
}

impl From<io::Error> for Error {
    fn from(x: io::Error) -> Self {
        Self::IOError(x)
    }
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

fn md5_file(filename: &str) -> Result<Vec<u8>, Error> {
    let mut file = File::open(filename)?;
    let file_size: usize = file.metadata()?.len() as usize;
    let mut hasher = Md5::new();

    if file_size <= BLOCK_SIZE {
        let mut buf: Vec<u8> = vec![0; file_size];
        let n = file.read(&mut buf)?;
        if n != file_size {
            return Err(Error::BufReadWrong);
        }
        hasher.update(&buf);
    } else {
        let mut buf: Vec<u8> = vec![0; BLOCK_SIZE];
        loop {
            let n = file.read(&mut buf)?;
            if n >= BLOCK_SIZE {
                hasher.update(&buf);
            } else {
                buf.resize(n, 0);
                hasher.update(&buf);
                break;
            }
        }
    }
    Ok(hasher.finalize().to_vec())
}

fn md5_file_parser(filename: &str) -> Result<(HashMap<String, String>, Vec<usize>), Error> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut dict: HashMap<String, String> = HashMap::new();
    let mut unreadable = vec![];

    for (i, line) in reader.lines().enumerate() {
        if let Ok(line) = line {
            if line.starts_with(';') || line.is_empty() {
                continue;
            }
            let temp: Vec<&str> = line.split(" *").collect();
            dict.insert(temp[1].to_string(), temp[0].to_string());
        } else {
            unreadable.push(i);
        }
    }
    Ok((dict, unreadable))
}

fn md5_checker(md5_list: HashMap<String, String>, thread_num: usize) {
    let mut temp_list: Vec<String> = md5_list.clone().into_keys().collect();
    temp_list.sort_unstable();

    let queue = VecDeque::from(temp_list);
    let mutex_queue = Arc::new(Mutex::new(queue));
    let mut handle_list = vec![];

    let (tx, rx) = mpsc::channel();

    for _ in 0..thread_num {
        let tx1 = tx.clone();
        let clone_queue = mutex_queue.clone();
        let handle = thread::spawn(move || loop {
            let filename: String;
            {
                let mut queue = clone_queue.lock().unwrap();
                if queue.is_empty() {
                    return;
                }
                filename = queue.pop_front().unwrap();
            }
            let hash = md5_file(&filename).unwrap_or(vec![]);
            tx1.send((filename, hash.to_hex())).unwrap();
        });
        handle_list.push(handle);
    }
    drop(tx);

    let mut correct_count = 0;
    let mut wrong_list = vec![];
    for mess in rx {
        if md5_list.get(&mess.0).unwrap() == &mess.1 {
            correct_count += 1;
            println!("{} {}", mess.0, "correct".bright_green());
        } else {
            wrong_list.push(mess.0.clone());
            println!("{} {}", mess.0, "wrong".bright_red());
        }
    }

    let total_count = correct_count + wrong_list.len();
    println!("Correct: {correct_count}/{total_count}");
    println!("Wrong: {}/{total_count}", wrong_list.len());
    for filename in wrong_list {
        println!("{} {}", filename, "wrong".bright_red());
    }
}

fn md5_checker_rayon(md5_list: HashMap<String, String>) {
    let wrong_list = Mutex::new(vec![]);
    md5_list.par_iter().for_each(|(filename, hash)| {
        let check_hash = md5_file(filename).unwrap_or(vec![]).to_hex();
        if check_hash == *hash {
            println!("{} {}", filename, "correct".bright_green());
        } else {
            let mut list = wrong_list.lock().unwrap();
            list.push(filename.clone());
            println!("{} {}", filename, "wrong".bright_red());
        }
    });
    let wrong_list = wrong_list.lock().unwrap();
    let total_count = md5_list.len();
    let correct_count = total_count - wrong_list.len();
    println!("Correct: {correct_count}/{total_count}");
    println!("Wrong: {}/{total_count}", wrong_list.len());
    for filename in wrong_list.iter() {
        println!("{} {}", filename, "wrong".bright_red());
    }
}

fn main() {
    let args = Args::parse();
    // let args = Args {
    //     rayon: true,
    //     filename: String::from("fitgirl.md5"),
    //     thread_num: 4,
    //     benchmark: false,
    // };
    let (list, unreadable_list) = md5_file_parser(&args.filename).unwrap();

    let before = Instant::now();
    if !args.rayon {
        md5_checker(list, args.thread_num);
    } else {
        md5_checker_rayon(list);
    }
    for n in unreadable_list {
        println!("Cannot read line at {}", n + 1);
    }
    if args.benchmark {
        println!("Elapsed time: {:.2?}", before.elapsed());
    }

    // let list = md5_file_parser("fitgirl.md5").unwrap();
    // println!("{:?}", list);
}
