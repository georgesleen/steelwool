//! Golden outputs plus the two invariants, run over every input in the suite.

use std::path::{Path, PathBuf};

use steel_parser::lexer::TokenStream;
use steel_parser::tokens::TokenType;
use steelwool::config::{Config, Layers};
use steelwool::verify::{check_equivalence, check_idempotence};
use steelwool::{Error, format_source};

fn data_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn corpus_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn inputs(directory: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(directory)
        .expect("test directory is readable")
        .map(|entry| entry.expect("directory entry is readable").path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "scm"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "{} has no inputs", directory.display());
    paths
}

/// Each input may sit next to a `.toml` file holding its configuration.
fn config_for(input: &Path) -> Config {
    let sidecar = input.with_extension("toml");
    if !sidecar.is_file() {
        return Config::default();
    }
    let mut layers = Layers::new();
    layers.push_file(&sidecar).expect("sidecar config parses");
    layers.resolve().expect("sidecar config resolves")
}

#[test]
fn golden_outputs_match() {
    let bless = std::env::var_os("STEELWOOL_BLESS").is_some();
    let mut failures = Vec::new();
    for input in inputs(&data_directory()) {
        let source =
            std::fs::read_to_string(&input).expect("input is readable");
        let formatted = format_source(&source, &config_for(&input))
            .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
        let expected_path = input.with_extension("expected");
        if bless {
            std::fs::write(&expected_path, &formatted)
                .expect("expected file is writable");
            continue;
        }
        let expected =
            std::fs::read_to_string(&expected_path).unwrap_or_else(|_| {
                panic!("{} is missing", expected_path.display())
            });
        if formatted != expected {
            failures.push(format!(
                "{}\n--- expected ---\n{expected}--- actual ---\n{formatted}",
                input.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Formatting is stable on a second pass and the golden output is a fixed
/// point, for every input in the suite including the real cogs.
#[test]
fn formatting_is_idempotent() {
    for directory in [data_directory(), corpus_directory()] {
        for input in inputs(&directory) {
            let source =
                std::fs::read_to_string(&input).expect("input is readable");
            let config = config_for(&input);
            let once = check_idempotence(&source, &config)
                .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
            let again = check_idempotence(&once, &config)
                .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
            assert_eq!(once, again, "{}", input.display());
        }
    }
}

/// Formatting never changes the program, for every input in the suite.
#[test]
fn formatting_preserves_the_program() {
    for directory in [data_directory(), corpus_directory()] {
        for input in inputs(&directory) {
            let source =
                std::fs::read_to_string(&input).expect("input is readable");
            let formatted = format_source(&source, &config_for(&input))
                .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
            check_equivalence(&source, &formatted)
                .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
        }
    }
}

/// Lines that may exceed the width: the formatter never rewraps comment text,
/// never moves a trailing comment off its line, and cannot split an atom.
fn excusable_lines(formatted: &str, width: usize) -> Vec<bool> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(formatted.match_indices('\n').map(|(index, _)| index + 1))
        .collect();
    let line_of = |offset: usize| match starts.binary_search(&offset) {
        Ok(line) => line,
        Err(next) => next - 1,
    };
    let mut excusable = vec![false; starts.len()];
    for token in TokenStream::new(formatted, false, None) {
        let token = token.expect("formatted output lexes");
        let first = line_of(token.span.start() as usize);
        let last = line_of(token.span.end().saturating_sub(1) as usize);
        let indent = formatted[starts[first]..]
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        let excused = token.ty == TokenType::Comment
            || last > first
            || indent + token.source.chars().count() > width;
        for line in first..=last.min(excusable.len() - 1) {
            excusable[line] |= excused;
        }
    }
    excusable
}

/// No line exceeds the configured width unless it provably cannot be split.
#[test]
fn code_lines_respect_the_width() {
    for directory in [data_directory(), corpus_directory()] {
        for input in inputs(&directory) {
            let source =
                std::fs::read_to_string(&input).expect("input is readable");
            let config = config_for(&input);
            let formatted = format_source(&source, &config)
                .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
            let excusable = excusable_lines(&formatted, config.width);
            for (number, line) in formatted.lines().enumerate() {
                if line.chars().count() <= config.width {
                    continue;
                }
                assert!(
                    excusable[number],
                    "{}:{}: {} columns: {line}",
                    input.display(),
                    number + 1,
                    line.chars().count()
                );
            }
        }
    }
}

/// A region between `fmt: off` and `fmt: on` survives byte for byte.
#[test]
fn fmt_off_regions_survive_unchanged() {
    let input = data_directory().join("fmt-off.scm");
    let source = std::fs::read_to_string(&input).expect("input is readable");
    let formatted =
        format_source(&source, &Config::default()).expect("input formats");
    for region in ["( 1  0  0 )", "'((a   1)\n    (bb  2))"] {
        assert!(
            formatted.contains(region),
            "lost `{region}` from a fmt: off region:\n{formatted}"
        );
    }
    assert!(
        formatted.contains("(define squashed (+ 1 2))"),
        "code outside the fmt: off regions was not formatted:\n{formatted}"
    );
}

#[test]
fn unbalanced_input_is_rejected() {
    let cases = [
        ("(define x", Error::UnclosedList { offset: 0 }),
        ("x)", Error::UnexpectedClose { offset: 1 }),
        ("(define x]", Error::MismatchedDelimiter { offset: 9 }),
        ("(list) '", Error::DanglingPrefix { offset: 7 }),
    ];
    for (source, expected) in cases {
        let error = format_source(source, &Config::default())
            .expect_err("unbalanced input must not format");
        assert_eq!(error, expected, "for input {source:?}");
    }
}
