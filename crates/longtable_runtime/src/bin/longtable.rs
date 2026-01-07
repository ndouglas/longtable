//! Longtable CLI entry point.

use longtable_runtime::Repl;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

/// CLI configuration parsed from arguments.
#[derive(Default, Debug)]
struct CliConfig {
    files: Vec<PathBuf>,
    batch_mode: bool,
    show_help: bool,
    show_version: bool,
    // Debug flags
    trace_rules: bool,
    trace_vm: bool,
    trace_match: bool,
    max_ticks: Option<u64>,
    dump_world: bool,
}

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

fn parse_args(args: Vec<String>) -> Result<CliConfig, Box<dyn std::error::Error>> {
    let mut config = CliConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => config.show_help = true,
            "-V" | "--version" => config.show_version = true,
            "-b" | "--batch" => config.batch_mode = true,
            "--trace" => config.trace_rules = true,
            "--trace-vm" => config.trace_vm = true,
            "--trace-match" => config.trace_match = true,
            "--dump-world" => config.dump_world = true,
            "--max-ticks" => {
                i += 1;
                if i >= args.len() {
                    return Err("--max-ticks requires a value".into());
                }
                config.max_ticks = Some(
                    args[i]
                        .parse()
                        .map_err(|_| format!("invalid --max-ticks value: {}", args[i]))?,
                );
            }
            arg if arg.starts_with('-') => {
                return Err(format!("unknown option: {arg}").into());
            }
            path => config.files.push(PathBuf::from(path)),
        }
        i += 1;
    }

    Ok(config)
}

fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args(args)?;

    if config.show_help {
        print_help();
        return Ok(());
    }

    if config.show_version {
        println!("longtable {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Print debug flag status if any are enabled
    if config.trace_rules || config.trace_vm || config.trace_match {
        eprintln!("\x1b[33mDebug flags enabled:\x1b[0m");
        if config.trace_rules {
            eprintln!("  - Rule tracing (--trace)");
        }
        if config.trace_vm {
            eprintln!("  - VM instruction tracing (--trace-vm)");
        }
        if config.trace_match {
            eprintln!("  - Pattern match tracing (--trace-match)");
        }
        if let Some(max) = config.max_ticks {
            eprintln!("  - Max ticks: {max}");
        }
        eprintln!();
    }

    // Create REPL
    let mut repl = Repl::new()?;

    // Load any specified files
    for file in &config.files {
        repl.eval_file(file)?;
    }

    // Dump world state if requested
    if config.dump_world {
        dump_world_state(repl.session().world());
    }

    // If batch mode, exit now
    if config.batch_mode {
        return Ok(());
    }

    // Run interactive REPL
    // If files were loaded, suppress banner since context is established
    if !config.files.is_empty() {
        repl = repl.without_banner();
    }

    repl.run()?;
    Ok(())
}

fn dump_world_state(world: &longtable_storage::World) {
    println!("\x1b[1;36m=== World State ===\x1b[0m");
    println!("Tick: {}", world.tick());
    println!("Seed: {}", world.seed());
    println!("Entities: {}", world.entity_count());

    // List all entities
    for entity in world.entities() {
        println!("  - {entity}");
    }

    println!();
}

fn print_help() {
    println!(
        "\x1b[1mLongtable\x1b[0m - Rule-based simulation engine

\x1b[1mUSAGE:\x1b[0m
    longtable [OPTIONS] [FILES...]

\x1b[1mARGUMENTS:\x1b[0m
    [FILES...]    Files to load before starting REPL

\x1b[1mOPTIONS:\x1b[0m
    -h, --help         Print help information
    -V, --version      Print version information
    -b, --batch        Load files and exit (no REPL)

\x1b[1mDEBUG OPTIONS:\x1b[0m
    --trace            Enable rule tracing output
    --trace-vm         Enable VM instruction tracing
    --trace-match      Enable pattern match tracing
    --max-ticks N      Limit ticks before exit (for testing)
    --dump-world       Dump world state after loading files

\x1b[1mEXAMPLES:\x1b[0m
    longtable                        Start interactive REPL
    longtable world.lt               Load world.lt, then start REPL
    longtable -b test.lt             Load test.lt and exit
    longtable components.lt rules.lt Load multiple files
    longtable --trace -b sim.lt      Run with rule tracing

\x1b[1mREPL COMMANDS:\x1b[0m
    (def name value)     Define a session variable
    (load \"path\")        Load a .lt file
    (save! \"path\")       Save world state to file
    (load-world! \"path\") Load world state from file
    (tick!)              Advance simulation by one tick
    (inspect entity)     Inspect an entity's details
    Ctrl+D               Exit REPL
    Ctrl+C               Cancel current input

For more information, visit https://github.com/ndouglas/longtable"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    // ==================== CLI Argument Parsing Tests ====================

    #[test]
    fn parse_empty_args() {
        let config = parse_args(args("longtable")).unwrap();
        assert!(config.files.is_empty());
        assert!(!config.batch_mode);
        assert!(!config.show_help);
        assert!(!config.show_version);
    }

    #[test]
    fn parse_help_short() {
        let config = parse_args(args("longtable -h")).unwrap();
        assert!(config.show_help);
    }

    #[test]
    fn parse_help_long() {
        let config = parse_args(args("longtable --help")).unwrap();
        assert!(config.show_help);
    }

    #[test]
    fn parse_version_short() {
        let config = parse_args(args("longtable -V")).unwrap();
        assert!(config.show_version);
    }

    #[test]
    fn parse_version_long() {
        let config = parse_args(args("longtable --version")).unwrap();
        assert!(config.show_version);
    }

    #[test]
    fn parse_batch_short() {
        let config = parse_args(args("longtable -b")).unwrap();
        assert!(config.batch_mode);
    }

    #[test]
    fn parse_batch_long() {
        let config = parse_args(args("longtable --batch")).unwrap();
        assert!(config.batch_mode);
    }

    #[test]
    fn parse_single_file() {
        let config = parse_args(args("longtable test.lt")).unwrap();
        assert_eq!(config.files.len(), 1);
        assert_eq!(config.files[0], PathBuf::from("test.lt"));
    }

    #[test]
    fn parse_multiple_files() {
        let config = parse_args(args("longtable components.lt rules.lt world.lt")).unwrap();
        assert_eq!(config.files.len(), 3);
        assert_eq!(config.files[0], PathBuf::from("components.lt"));
        assert_eq!(config.files[1], PathBuf::from("rules.lt"));
        assert_eq!(config.files[2], PathBuf::from("world.lt"));
    }

    #[test]
    fn parse_batch_with_file() {
        let config = parse_args(args("longtable -b test.lt")).unwrap();
        assert!(config.batch_mode);
        assert_eq!(config.files.len(), 1);
    }

    #[test]
    fn parse_trace_flags() {
        let config = parse_args(args("longtable --trace --trace-vm --trace-match")).unwrap();
        assert!(config.trace_rules);
        assert!(config.trace_vm);
        assert!(config.trace_match);
    }

    #[test]
    fn parse_dump_world() {
        let config = parse_args(args("longtable --dump-world")).unwrap();
        assert!(config.dump_world);
    }

    #[test]
    fn parse_max_ticks() {
        let config = parse_args(args("longtable --max-ticks 100")).unwrap();
        assert_eq!(config.max_ticks, Some(100));
    }

    #[test]
    fn parse_max_ticks_with_file() {
        let config = parse_args(args("longtable --max-ticks 50 -b test.lt")).unwrap();
        assert_eq!(config.max_ticks, Some(50));
        assert!(config.batch_mode);
        assert_eq!(config.files.len(), 1);
    }

    #[test]
    fn parse_unknown_option_fails() {
        let result = parse_args(args("longtable --unknown"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown option"));
    }

    #[test]
    fn parse_max_ticks_missing_value_fails() {
        let result = parse_args(args("longtable --max-ticks"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a value"));
    }

    #[test]
    fn parse_max_ticks_invalid_value_fails() {
        let result = parse_args(args("longtable --max-ticks abc"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid"));
    }

    #[test]
    fn parse_combined_options() {
        let config = parse_args(args(
            "longtable -b --trace --dump-world --max-ticks 10 test.lt",
        ))
        .unwrap();
        assert!(config.batch_mode);
        assert!(config.trace_rules);
        assert!(config.dump_world);
        assert_eq!(config.max_ticks, Some(10));
        assert_eq!(config.files.len(), 1);
    }

    // ==================== File Execution Tests ====================
    // Note: These tests verify the run function behavior

    #[test]
    fn run_help_does_not_error() {
        // Help should succeed without needing any files
        let result = run(args("longtable --help"));
        assert!(result.is_ok());
    }

    #[test]
    fn run_version_does_not_error() {
        // Version should succeed without needing any files
        let result = run(args("longtable --version"));
        assert!(result.is_ok());
    }

    #[test]
    fn run_nonexistent_file_fails() {
        // Loading a non-existent file should fail
        let result = run(args("longtable -b /nonexistent/path/to/file.lt"));
        assert!(result.is_err());
    }

    #[test]
    fn run_batch_no_files_succeeds() {
        // Batch mode with no files should succeed (just starts and exits)
        let result = run(args("longtable -b"));
        assert!(result.is_ok());
    }
}
