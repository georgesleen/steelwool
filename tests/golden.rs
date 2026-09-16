//! Golden outputs plus the two invariants, run over every input in the suite.

use std::path::{Path, PathBuf};

use steel_parser::lexer::TokenStream;
use steel_parser::tokens::TokenType;
use steelwool::config::{Config, Layers};
use steelwool::verify::{check_equivalence, check_idempotence};
use steelwool::{Error, format_source};

mod support;

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

/// The equivalence check has to reject changes, including comment changes: a
/// `;;@doc` block becomes an `@doc` form in the Steel AST.
#[test]
fn the_equivalence_check_rejects_a_changed_program() {
    let changes = [
        ("(define x 1)", "(define x 2)"),
        ("(list a b)", "(list b a)"),
        ("(define x 1) ;; note", "(define x 1)"),
        (
            ";;@doc\n;; Adds.\n(define (f a) a)",
            ";;@doc\n;; Subtracts.\n(define (f a) a)",
        ),
        ("(require \"a.scm\" \"b.scm\")", "(require \"a.scm\")"),
        (";; note", ";; other note"),
        (";;@doc x", ";; @doc x"),
        ("(quote a b)", "(quote b a)"),
        ("'(a)", "'(b)"),
        ("(list #t)", "(list #t #t)"),
    ];
    for (before, after) in changes {
        check_equivalence(before, after).expect_err(&format!(
            "{before:?} and {after:?} must not compare equal"
        ));
    }

    // The rewrites the passes are allowed to make, oracle rules R1 to R5.
    let rewrites = [
        (
            "(require \"b.scm\" \"a.scm\")",
            "(require \"a.scm\" \"b.scm\")",
        ),
        ("(provide b a)", "(provide a b)"),
        (";one", ";; one"),
        ("(quote (a b))", "'(a b)"),
        ("(quasiquote (a (unquote b)))", "`(a ,b)"),
        ("(list #true #false)", "(list #t #f)"),
    ];
    for (before, after) in rewrites {
        check_equivalence(before, after).unwrap_or_else(|error| {
            panic!("{before:?} and {after:?} are the same program: {error}")
        });
    }
}

/// Every input in the suite, formatted with its own configuration.
fn suite() -> Vec<(PathBuf, Config, String)> {
    let mut formatted = Vec::new();
    for directory in [data_directory(), corpus_directory()] {
        for input in inputs(&directory) {
            let source =
                std::fs::read_to_string(&input).expect("input is readable");
            let config = config_for(&input);
            let output = format_source(&source, &config)
                .unwrap_or_else(|error| panic!("{}: {error}", input.display()));
            formatted.push((input, config, output));
        }
    }
    formatted
}

/// No line exceeds the configured width unless it provably cannot be split.
#[test]
fn code_lines_respect_the_width() {
    for (input, config, formatted) in suite() {
        if let Some((number, line)) =
            support::unexcused_overruns(&formatted, config.width)
                .into_iter()
                .next()
        {
            panic!(
                "{}:{number}: {} columns: {line}",
                input.display(),
                line.chars().count()
            );
        }
    }
}

/// Rule L3.
#[test]
fn output_ends_with_one_newline() {
    for (input, _, formatted) in suite() {
        assert!(
            formatted.ends_with('\n') && !formatted.ends_with("\n\n"),
            "{}",
            input.display()
        );
    }
    assert_eq!(
        format_source("", &Config::default()).expect("empty input formats"),
        ""
    );
}

/// Rule L4.
#[test]
fn no_line_ends_in_whitespace() {
    for (input, _, formatted) in suite() {
        for (number, line) in formatted.lines().enumerate() {
            assert_eq!(
                line.trim_end(),
                line,
                "{}:{}",
                input.display(),
                number + 1
            );
        }
    }
}

/// Rule L5. A tab is only ever passed through from a string literal or from
/// comment text, neither of which the formatter rewrites.
#[test]
fn no_tabs_outside_strings_and_comments() {
    for (input, _, formatted) in suite() {
        let mut allowed = vec![false; formatted.len()];
        for token in TokenStream::new(&formatted, false, None) {
            let token = token.expect("formatted output lexes");
            if !matches!(
                token.ty,
                TokenType::StringLiteral(_) | TokenType::Comment
            ) {
                continue;
            }
            let span = token.span.start() as usize..token.span.end() as usize;
            allowed[span].fill(true);
        }
        for (offset, _) in formatted.match_indices('\t') {
            assert!(allowed[offset], "{}:{offset}", input.display());
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
