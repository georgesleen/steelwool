use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use steelwool::config::{Config, Layers};
use steelwool::{Error, diagnostics, diff, format_source};

const USAGE: &str = "\
steelwool, an opinionated formatter for Steel

Usage: steelwool [OPTIONS] [FILES...]

With no FILES, or with `-`, reads stdin and writes stdout.

Options:
  -                    Format stdin and write the result to stdout
      --check          Write nothing, exit 1 if any input would change
      --diff           Write nothing, print a unified diff of each change
      --list-different Write nothing, print the path of each changed input
      --config PATH    Use PATH instead of searching for a config file
      --config-toml S  Apply S as a TOML configuration layer
      --set KEY=VALUE  Override one dotted key with a TOML value, repeatable
  -h, --help           Print this help
  -V, --version        Print the version
";

const STDIN: &str = "<stdin>";

struct Arguments {
    files: Vec<PathBuf>,
    stdin: bool,
    check: bool,
    diff: bool,
    list_different: bool,
    config: Option<PathBuf>,
    overlays: Vec<Overlay>,
}

impl Arguments {
    /// True in the modes that report what would change instead of doing it.
    fn writes_nothing(&self) -> bool {
        self.check || self.diff || self.list_different
    }
}

enum Overlay {
    Toml(String),
    Set(String),
}

fn parse_arguments(
    raw: impl Iterator<Item = String>,
) -> Result<Arguments, String> {
    let mut arguments = Arguments {
        files: Vec::new(),
        stdin: false,
        check: false,
        diff: false,
        list_different: false,
        config: None,
        overlays: Vec::new(),
    };
    let mut raw = raw.peekable();
    let mut options_ended = false;
    while let Some(argument) = raw.next() {
        if options_ended {
            arguments.files.push(PathBuf::from(argument));
            continue;
        }
        match argument.as_str() {
            "--" => options_ended = true,
            "-" => arguments.stdin = true,
            "--check" => arguments.check = true,
            "--diff" => arguments.diff = true,
            "--list-different" => arguments.list_different = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("steelwool {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--config" => {
                let value = raw.next().ok_or("--config needs a path")?;
                arguments.config = Some(PathBuf::from(value));
            }
            "--config-toml" => {
                let value = raw.next().ok_or("--config-toml needs a value")?;
                arguments.overlays.push(Overlay::Toml(value));
            }
            "--set" => {
                let value = raw.next().ok_or("--set needs KEY.PATH=VALUE")?;
                arguments.overlays.push(Overlay::Set(value));
            }
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option `{other}`"));
            }
            other => arguments.files.push(PathBuf::from(other)),
        }
    }
    Ok(arguments)
}

/// Builds the configuration for one input, discovering a file when none was
/// given explicitly.
fn resolve(
    arguments: &Arguments,
    start: &Path,
) -> Result<Config, Box<dyn std::error::Error>> {
    let mut layers = Layers::new();
    match &arguments.config {
        Some(path) => layers.push_file(path)?,
        None => {
            if let Some(path) = steelwool::config::discover(start) {
                layers.push_file(&path)?;
            }
        }
    }
    for overlay in &arguments.overlays {
        match overlay {
            Overlay::Toml(text) => layers.push_toml(text, "--config-toml")?,
            Overlay::Set(argument) => layers.push_set(argument)?,
        }
    }
    Ok(layers.resolve()?)
}

fn directory_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// What one input turned out to need, decided on a worker thread and acted
/// on later in argument order.
enum Outcome {
    Unchanged,
    Changed { source: String, formatted: String },
    Failed(String),
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("steelwool: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if arguments.stdin || arguments.files.is_empty() {
        return format_stdin(&arguments);
    }
    Ok(report(&arguments, format_files(&arguments)))
}

/// Formats every input on up to `available_parallelism` threads, returning
/// one outcome per input in argument order.
fn format_files(arguments: &Arguments) -> Vec<Outcome> {
    let workers = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1)
        .min(arguments.files.len());
    let next = AtomicUsize::new(0);
    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = arguments.files.get(index) else {
                        break;
                    };
                    sender
                        .send((index, format_one(arguments, path)))
                        .expect("the receiver outlives the workers");
                }
            });
        }
        drop(sender);
    });

    let mut outcomes: Vec<Option<Outcome>> =
        arguments.files.iter().map(|_| None).collect();
    for (index, outcome) in receiver {
        outcomes[index] = Some(outcome);
    }
    outcomes
        .into_iter()
        .map(|outcome| outcome.expect("every input is visited once"))
        .collect()
}

/// Reads, configures and formats one input. Everything that can go wrong is
/// turned into a finished diagnostic here, on the worker.
fn format_one(arguments: &Arguments, path: &Path) -> Outcome {
    let shown = path.display().to_string();
    let config = match resolve(arguments, &directory_of(path)) {
        Ok(config) => config,
        Err(error) => return Outcome::Failed(format!("{shown}: {error}")),
    };
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => return Outcome::Failed(format!("{shown}: {error}")),
    };
    match format_source(&source, &config) {
        Ok(formatted) if formatted == source => Outcome::Unchanged,
        Ok(formatted) => Outcome::Changed { source, formatted },
        Err(error) => Outcome::Failed(diagnostic(&shown, &source, &error)),
    }
}

/// `PATH:LINE:COLUMN: message` for an error that knows where it is, and
/// `PATH: message` for one that does not.
fn diagnostic(path: &str, source: &str, error: &Error) -> String {
    match error.offset() {
        Some(offset) => {
            let at = diagnostics::position(source, offset);
            format!("{path}:{}:{}: {error}", at.line, at.column)
        }
        None => format!("{path}: {error}"),
    }
}

/// Acts on the outcomes in argument order: writes, diffs, paths, failures.
fn report(arguments: &Arguments, outcomes: Vec<Outcome>) -> ExitCode {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    let mut changed = false;
    let mut failed = false;

    for (path, outcome) in arguments.files.iter().zip(outcomes) {
        let shown = path.display().to_string();
        match outcome {
            Outcome::Unchanged => {}
            Outcome::Failed(message) => {
                failed = true;
                eprintln!("steelwool: {message}");
            }
            Outcome::Changed { source, formatted } => {
                changed = true;
                if !arguments.writes_nothing() {
                    if let Err(error) = std::fs::write(path, &formatted) {
                        failed = true;
                        eprintln!("steelwool: {shown}: {error}");
                    }
                    continue;
                }
                if arguments.diff {
                    let diff = diff::unified(&shown, &source, &formatted);
                    let _ = stdout.write_all(diff.as_bytes());
                }
                if arguments.list_different {
                    let _ = writeln!(stdout, "{shown}");
                }
                if arguments.check {
                    eprintln!("steelwool: would reformat {shown}");
                }
            }
        }
    }
    let _ = stdout.flush();

    exit_code(failed, changed && arguments.writes_nothing())
}

fn exit_code(failed: bool, would_change: bool) -> ExitCode {
    if failed {
        ExitCode::from(2)
    } else if would_change {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn format_stdin(
    arguments: &Arguments,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let config = resolve(arguments, &std::env::current_dir()?)
        .map_err(|error| format!("{STDIN}: {error}"))?;
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let formatted = match format_source(&source, &config) {
        Ok(formatted) => formatted,
        Err(error) => {
            eprintln!("steelwool: {}", diagnostic(STDIN, &source, &error));
            return Ok(ExitCode::from(2));
        }
    };

    if !arguments.writes_nothing() {
        std::io::stdout().write_all(formatted.as_bytes())?;
        return Ok(ExitCode::SUCCESS);
    }
    if formatted == source {
        return Ok(ExitCode::SUCCESS);
    }

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    if arguments.diff {
        let diff = diff::unified(STDIN, &source, &formatted);
        stdout.write_all(diff.as_bytes())?;
    }
    if arguments.list_different {
        writeln!(stdout, "{STDIN}")?;
    }
    stdout.flush()?;
    if arguments.check {
        eprintln!("steelwool: would reformat {STDIN}");
    }
    Ok(ExitCode::FAILURE)
}
