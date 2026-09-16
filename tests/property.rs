//! Properties over generated Steel source: rules V5 to V8.

use proptest::prelude::*;
use steelwool::Config;
use steelwool::format_source;
use steelwool::verify::check_idempotence;

mod support;

/// Atoms the generator draws from. `as` and `quote` are here because they
/// switch passes on and off: `as` blocks a sort, `quote` invites shorthand.
const ATOMS: [&str; 22] = [
    "x",
    "y",
    "value",
    "define",
    "lambda",
    "let",
    "cond",
    "else",
    "begin",
    "when",
    "require",
    "provide",
    "quote",
    "quasiquote",
    "as",
    "1",
    "42",
    "#t",
    "#true",
    "#false",
    "\"a string\"",
    "a-deliberately-long-identifier-that-will-not-fit-on-one-line",
];

const COMMENTS: [&str; 6] = [
    ";; note",
    "; note",
    ";;;; banner",
    ";;@doc",
    ";;",
    ";;    indented note",
];

const MARKERS: [&str; 4] = ["'", "`", ",", ",@"];

#[derive(Debug, Clone)]
enum Datum {
    Atom(&'static str),
    Comment(&'static str),
    List {
        square: bool,
        items: Vec<Datum>,
    },
    Quoted {
        marker: &'static str,
        target: Box<Datum>,
    },
}

impl Datum {
    fn is_comment(&self) -> bool {
        matches!(self, Datum::Comment(_))
    }
}

fn datum(atoms: &'static [&'static str]) -> impl Strategy<Value = Datum> {
    let leaf = prop_oneof![
        5 => prop::sample::select(atoms).prop_map(Datum::Atom),
        1 => prop::sample::select(COMMENTS.as_slice())
            .prop_map(Datum::Comment),
    ];
    leaf.prop_recursive(4, 48, 5, |inner| {
        prop_oneof![
            3 => (any::<bool>(), prop::collection::vec(inner.clone(), 0..5))
                .prop_map(|(square, items)| Datum::List { square, items }),
            1 => (prop::sample::select(MARKERS.as_slice()), inner).prop_map(
                |(marker, target)| Datum::Quoted {
                    marker,
                    // A prefix needs a datum, and a comment is not one.
                    target: Box::new(match target {
                        Datum::Comment(_) => Datum::Atom("x"),
                        other => other,
                    }),
                }
            ),
        ]
    })
}

/// A whole source file: top level datums, the whitespace to separate them
/// with, and the width to format at.
#[derive(Debug, Clone)]
struct Source {
    datums: Vec<Datum>,
    gaps: Vec<u8>,
    width: usize,
}

fn source(
    atoms: &'static [&'static str],
    widths: std::ops::Range<usize>,
) -> impl Strategy<Value = Source> {
    (
        prop::collection::vec(datum(atoms), 1..8),
        prop::collection::vec(any::<u8>(), 1..24),
        widths,
    )
        .prop_map(|(datums, gaps, width)| Source {
            datums,
            gaps,
            width,
        })
}

/// Source that may hold an atom no line can fit.
fn any_source() -> impl Strategy<Value = Source> {
    source(ATOMS.as_slice(), 20..100)
}

/// Source whose every atom fits at the width it is formatted at, so any
/// overrun beyond rule L17's excuses is the layout engine's fault.
fn fitting_source() -> impl Strategy<Value = Source> {
    source(&ATOMS[..ATOMS.len() - 1], 60..100)
}

impl Source {
    fn text(&self) -> String {
        let mut out = String::new();
        let mut gaps = self.gaps.iter().copied().cycle();
        for (index, datum) in self.datums.iter().enumerate() {
            if index > 0 {
                separate(&mut out, gaps.next().unwrap_or(0), true);
            }
            write(&mut out, datum, &mut gaps);
        }
        out.push('\n');
        out
    }
}

/// One space, one newline, or a blank line. A comment swallows the rest of
/// its line, so anything after one starts on a new line.
fn separate(out: &mut String, gap: u8, after_comment: bool) {
    match gap % 3 {
        0 if !after_comment => out.push(' '),
        1 => out.push('\n'),
        _ => out.push_str("\n\n"),
    }
    if out.ends_with('\n') {
        for _ in 0..(gap % 5) {
            out.push(' ');
        }
    }
}

fn write(out: &mut String, datum: &Datum, gaps: &mut impl Iterator<Item = u8>) {
    match datum {
        Datum::Atom(text) => out.push_str(text),
        Datum::Comment(text) => out.push_str(text),
        Datum::Quoted { marker, target } => {
            out.push_str(marker);
            write(out, target, gaps);
        }
        Datum::List { square, items } => {
            out.push(if *square { '[' } else { '(' });
            let mut previous_comment = false;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    let gap = gaps.next().unwrap_or(0);
                    separate(out, gap, previous_comment);
                }
                write(out, item, gaps);
                previous_comment = item.is_comment();
            }
            if previous_comment {
                out.push('\n');
            }
            out.push(if *square { ']' } else { ')' });
        }
    }
}

fn config_at(width: usize) -> Config {
    Config {
        width,
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    /// Rules V5 and V6: generated source formats, and formatting it is
    /// idempotent. Formatting checks equivalence itself, so a failure here is
    /// either a rejected program or an unstable one.
    #[test]
    fn generated_source_formats_and_is_stable(source in any_source()) {
        let text = source.text();
        let config = config_at(source.width);
        let formatted = check_idempotence(&text, &config)
            .map_err(|error| TestCaseError::fail(
                format!("{error}\n--- input ---\n{text}")
            ))?;
        prop_assert!(formatted.ends_with('\n'));
    }

    /// Rule V7: every line respects the width it was formatted at, except the
    /// overruns layout rule L17 permits, for source whose atoms fit at that
    /// width.
    #[test]
    fn generated_source_respects_the_width(source in fitting_source()) {
        let text = source.text();
        let config = config_at(source.width);
        let formatted = format_source(&text, &config)
            .map_err(|error| TestCaseError::fail(
                format!("{error}\n--- input ---\n{text}")
            ))?;
        let overruns =
            support::unexcused_overruns(&formatted, config.width);
        prop_assert!(
            overruns.is_empty(),
            "width {}: {overruns:?}\n--- input ---\n{text}\
             \n--- output ---\n{formatted}",
            config.width
        );
    }

    /// Rule V8: arbitrary text yields output or an error, never a panic.
    #[test]
    fn arbitrary_text_never_panics(
        text in r#"[()\[\]'`,@;#\\ \t\n"A-Za-z0-9]{0,120}"#
    ) {
        let _ = format_source(&text, &Config::default());
    }

    /// Rule V8, with no structure to lean on.
    #[test]
    fn arbitrary_unicode_never_panics(text in ".{0,120}") {
        let _ = format_source(&text, &Config::default());
    }
}
