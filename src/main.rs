use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use steelwool::config::{Config, Layers};
use steelwool::format_source;

const USAGE: &str = "\
steelwool, an opinionated formatter for Steel

Usage: steelwool [OPTIONS] [FILES...]

With no FILES, or with `-`, reads stdin and writes stdout.

Options:
  -                    Format stdin and write the result to stdout
      --check          Write nothing, exit 1 if any input would change
      --config PATH    Use PATH instead of searching for a config file
      --config-toml S  Apply S as a TOML configuration layer
      --set KEY=VALUE  Override one dotted key with a TOML value, repeatable
  -h, --help           Print this help
  -V, --version        Print the version
";

struct Arguments {
    files: Vec<PathBuf>,
    stdin: bool,
    check: bool,
    config: Option<PathBuf>,
    overlays: Vec<Overlay>,
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

    let mut would_change = Vec::new();
    for path in &arguments.files {
        let config = resolve(&arguments, &directory_of(path))?;
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let formatted = format_source(&source, &config)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if formatted == source {
            continue;
        }
        if arguments.check {
            would_change.push(path.clone());
        } else {
            std::fs::write(path, &formatted)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }

    if would_change.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    for path in &would_change {
        eprintln!("would reformat {}", path.display());
    }
    Ok(ExitCode::FAILURE)
}

fn format_stdin(
    arguments: &Arguments,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let config = resolve(arguments, &std::env::current_dir()?)?;
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let formatted = format_source(&source, &config)
        .map_err(|error| format!("<stdin>: {error}"))?;
    if arguments.check {
        if formatted == source {
            return Ok(ExitCode::SUCCESS);
        }
        eprintln!("would reformat <stdin>");
        return Ok(ExitCode::FAILURE);
    }
    std::io::stdout().write_all(formatted.as_bytes())?;
    Ok(ExitCode::SUCCESS)
}
