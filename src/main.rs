use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use duscope::{print_tree, scan};

struct Options {
    path: PathBuf,
    json: bool,
    depth: Option<usize>,
    top: Option<usize>,
    excludes: Vec<String>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut path = PathBuf::from(".");
    let mut path_set = false;
    let mut json = false;
    let mut depth = None;
    let mut top = None;
    let mut excludes = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json = true,
            "--depth" => {
                i += 1;
                let value = args.get(i).ok_or("--depth requires a number")?;
                depth = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid --depth value: {value}"))?,
                );
            }
            "--top" => {
                i += 1;
                let value = args.get(i).ok_or("--top requires a number")?;
                top = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid --top value: {value}"))?,
                );
            }
            "--exclude" => {
                i += 1;
                let value = args.get(i).ok_or("--exclude requires a pattern")?;
                excludes.push(value.clone());
            }
            other if !path_set && !other.starts_with('-') => {
                path = PathBuf::from(other);
                path_set = true;
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
        i += 1;
    }

    Ok(Options {
        path,
        json,
        depth,
        top,
        excludes,
    })
}

fn print_help() {
    println!("duscope - report disk usage for a directory tree");
    println!();
    println!("usage: duscope [path] [--json] [--depth N] [--top N] [--exclude PATTERN]...");
    println!();
    println!("  path              directory to scan (default: current directory)");
    println!("  --json            print machine-readable JSON instead of a tree");
    println!("  --depth N         only display N levels of the tree (scanning is always full)");
    println!("  --top N           only display the N largest entries per directory");
    println!("  --exclude PATTERN skip entries whose name matches PATTERN (glob, may repeat)");
}

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();

    if raw_args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return ExitCode::SUCCESS;
    }

    let options = match parse_args(&raw_args) {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("duscope: {msg}");
            return ExitCode::FAILURE;
        }
    };

    let entry = scan(&options.path, &options.excludes);

    if options.json {
        println!("{}", entry.to_json());
    } else {
        print_tree(&entry, 0, options.depth, options.top);
    }

    ExitCode::SUCCESS
}
