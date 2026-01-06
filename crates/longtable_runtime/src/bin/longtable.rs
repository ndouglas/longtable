//! Longtable CLI entry point.

use longtable_runtime::Repl;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\x1b[31mError: {e}\x1b[0m");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments
    let mut files: Vec<PathBuf> = Vec::new();
    let mut batch_mode = false;
    let mut show_help = false;
    let mut show_version = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => show_help = true,
            "-V" | "--version" => show_version = true,
            "-b" | "--batch" => batch_mode = true,
            arg if arg.starts_with('-') => {
                return Err(format!("unknown option: {arg}").into());
            }
            path => files.push(PathBuf::from(path)),
        }
        i += 1;
    }

    if show_help {
        print_help();
        return Ok(());
    }

    if show_version {
        println!("longtable {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Create REPL
    let mut repl = Repl::new()?;

    // Load any specified files
    for file in &files {
        repl.eval_file(file)?;
    }

    // If batch mode or files were specified without --batch, decide behavior
    if batch_mode {
        // Just load files and exit
        return Ok(());
    }

    // Run interactive REPL
    // If files were loaded, suppress banner since context is established
    if !files.is_empty() {
        repl = repl.without_banner();
    }

    repl.run()?;
    Ok(())
}

fn print_help() {
    println!(
        "\x1b[1mLongtable\x1b[0m - Rule-based simulation engine

\x1b[1mUSAGE:\x1b[0m
    longtable [OPTIONS] [FILES...]

\x1b[1mARGUMENTS:\x1b[0m
    [FILES...]    Files to load before starting REPL

\x1b[1mOPTIONS:\x1b[0m
    -h, --help       Print help information
    -V, --version    Print version information
    -b, --batch      Load files and exit (no REPL)

\x1b[1mEXAMPLES:\x1b[0m
    longtable                        Start interactive REPL
    longtable world.lt               Load world.lt, then start REPL
    longtable -b test.lt             Load test.lt and exit
    longtable components.lt rules.lt Load multiple files

\x1b[1mREPL COMMANDS:\x1b[0m
    (def name value)    Define a session variable
    (load \"path\")       Load a .lt file
    Ctrl+D              Exit REPL
    Ctrl+C              Cancel current input

For more information, visit https://github.com/ndouglas/longtable"
    );
}
