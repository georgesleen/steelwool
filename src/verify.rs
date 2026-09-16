//! The invariant that matters: formatting must never change the program.

use steel_parser::lexer::TokenStream;
use steel_parser::parser::Parser;
use steel_parser::tokens::TokenType;

use crate::config::Config;
use crate::cst::is_prefix;
use crate::layout::{REQUIRE_HEADS, is_doc_marker};
use crate::{Error, format_source};
/// A span-free datum tree, including comments.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Sexp {
    Atom(String),
    List(char, Vec<Sexp>),
}

/// A lexed file: its datum trees, and whether a long form `(quasiquote ...)`
/// appeared in it.
struct Lexed {
    forms: Vec<Sexp>,
    long_quasiquote: bool,
}

/// Lexes `source` into datum trees. Comments are datums here: a `;;@doc` block
/// becomes an `@doc` form in the Steel AST, so losing or rewording a comment
/// can change the program.
fn datums(source: &str) -> Result<Lexed, Error> {
    let mut long_quasiquote = false;
    let mut stack = vec![Frame::root()];
    for token in TokenStream::new(source, false, None) {
        let token = token.map_err(|error| Error::Lex {
            message: error.ty.to_string(),
            offset: error.span.start(),
        })?;
        let frame = stack.last_mut().expect("root frame is never popped");
        match &token.ty {
            TokenType::OpenParen(open, _) => stack.push(Frame {
                close: open.close(),
                items: Vec::new(),
                pending: Vec::new(),
            }),
            TokenType::CloseParen(close) => {
                if stack.len() == 1 {
                    return Err(Error::UnexpectedClose {
                        offset: token.span.start(),
                    });
                }
                let frame = stack.pop().expect("checked depth above");
                if frame.close != close.close() {
                    return Err(Error::MismatchedDelimiter {
                        offset: token.span.start(),
                    });
                }
                if !frame.pending.is_empty() {
                    return Err(Error::DanglingPrefix {
                        offset: token.span.start(),
                    });
                }
                long_quasiquote |= matches!(
                    frame.items.first(),
                    Some(Sexp::Atom(head)) if head == "quasiquote"
                );
                let datum = Sexp::List(frame.close, frame.items);
                stack
                    .last_mut()
                    .expect("root frame is never popped")
                    .push(datum);
            }
            // A comment does not satisfy a pending prefix.
            TokenType::Comment => {
                frame.items.push(Sexp::Atom(text_of(token.source)));
            }
            ty if is_prefix(ty) => {
                frame.pending.push(token.source.to_string());
            }
            _ => frame.push(Sexp::Atom(text_of(token.source))),
        }
    }
    if stack.len() != 1 {
        return Err(Error::UnclosedList { offset: 0 });
    }
    let root = stack.pop().expect("root frame is never popped");
    if !root.pending.is_empty() {
        return Err(Error::DanglingPrefix { offset: 0 });
    }
    Ok(Lexed {
        forms: root.items,
        long_quasiquote,
    })
}

fn text_of(source: &str) -> String {
    source.trim_end().to_string()
}

/// A stack frame: the delimiter it expects, the datums it has taken, and the
/// quote prefixes waiting for the next datum.
struct Frame {
    close: char,
    items: Vec<Sexp>,
    pending: Vec<String>,
}

impl Frame {
    fn root() -> Self {
        Frame {
            close: '\0',
            items: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Folds pending prefixes onto `datum`, so that rule R4 sees `'x` and
    /// `(quote x)` as the same datum.
    fn push(&mut self, mut datum: Sexp) {
        while let Some(marker) = self.pending.pop() {
            datum = Sexp::List(
                ')',
                vec![Sexp::Atom(prefix_head(&marker).to_string()), datum],
            );
        }
        self.items.push(datum);
    }
}

/// The long name of a quoting prefix. Any other prefix, `#;` among them, has
/// no long form and stands for itself.
fn prefix_head(marker: &str) -> &str {
    match marker {
        "'" => "quote",
        "`" => "quasiquote",
        "," => "unquote",
        ",@" => "unquote-splicing",
        other => other,
    }
}

fn head_name(sexp: &Sexp) -> Option<&str> {
    match sexp {
        Sexp::List(_, items) => match items.first() {
            Some(Sexp::Atom(text)) => Some(text.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// Applies the rewrites the formatter is allowed to perform, so that the
/// comparison ignores them on both sides. See `docs/spec/correctness.md`.
fn canonicalize(forms: &mut [Sexp]) {
    for form in forms.iter_mut() {
        canonicalize_nested(form);
    }
    let mut start = 0;
    while start < forms.len() {
        if !head_name(&forms[start]).is_some_and(|n| REQUIRE_HEADS.contains(&n))
        {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < forms.len()
            && head_name(&forms[end])
                .is_some_and(|n| REQUIRE_HEADS.contains(&n))
        {
            end += 1;
        }
        forms[start..end].sort();
        start = end;
    }
}

fn canonicalize_nested(form: &mut Sexp) {
    match form {
        Sexp::Atom(text) => canonicalize_atom(text),
        Sexp::List(_, items) => {
            for item in items.iter_mut() {
                canonicalize_nested(item);
            }
            sort_clauses(items);
        }
    }
}

/// Rules R3 and R5: a comment keeps its body and loses its marker, unless it
/// is a doc comment, whose marker is program text; a boolean keeps one
/// spelling.
fn canonicalize_atom(text: &mut String) {
    match text.as_str() {
        "#true" => *text = "#t".to_string(),
        "#false" => *text = "#f".to_string(),
        _ => {}
    }
    if text.starts_with(';') && !is_doc_marker(text) {
        *text = format!(";{}", text.trim_start_matches(';').trim());
    }
}

/// Rules R1 and R2: the inside of a `require` or a `provide` is a set, unless
/// a positional `as` or a comment among the clauses makes it a sequence.
fn sort_clauses(items: &mut [Sexp]) {
    let sortable = matches!(
        items.first(),
        Some(Sexp::Atom(head)) if head == "require" || head == "provide"
    );
    if !sortable || items.len() < 3 {
        return;
    }
    let positional_or_commented = items[1..].iter().any(|item| {
        matches!(item, Sexp::Atom(text) if text == "as" || is_comment(text))
    });
    if positional_or_commented {
        return;
    }
    items[1..].sort();
}

fn is_comment(text: &str) -> bool {
    text.starts_with(';') || text.starts_with("#|")
}

/// Fails when the formatted output is not the same program as the input.
pub fn check_equivalence(before: &str, after: &str) -> Result<(), Error> {
    let original = datums(before)?;
    let formatted = datums(after)?;
    let mut original_forms = original.forms;
    let mut formatted_forms = formatted.forms;
    canonicalize(&mut original_forms);
    canonicalize(&mut formatted_forms);
    if original_forms != formatted_forms {
        return Err(Error::ProgramChanged);
    }

    let Some(Ok(before_forms)) = parser_verdict(before) else {
        return Ok(());
    };
    match parser_verdict(after) {
        Some(Ok(after_forms)) if after_forms == before_forms => Ok(()),
        // The pinned parser leaves long-form quasiquote context dangling and
        // can therefore misread the next top-level form.
        _ if original.long_quasiquote => Ok(()),
        Some(Ok(after_forms)) => Err(Error::OutputRejected(format!(
            "{before_forms} top level forms became {after_forms}"
        ))),
        Some(Err(message)) => Err(Error::OutputRejected(message)),
        None => Err(Error::OutputRejected(
            "the Steel parser panicked on the output".to_string(),
        )),
    }
}

/// Runs the pinned parser without allowing its assertion failures to abort
/// steelwool.
fn parser_verdict(source: &str) -> Option<Result<usize, String>> {
    std::panic::catch_unwind(|| {
        Parser::new(source, None)
            .collect::<Result<Vec<_>, _>>()
            .map(|forms| forms.len())
            .map_err(|error| error.to_string())
    })
    .ok()
}

/// Fails when a second formatting pass would change the output again.
pub fn check_idempotence(
    source: &str,
    config: &Config,
) -> Result<String, Error> {
    let once = format_source(source, config)?;
    let twice = format_source(&once, config)?;
    if once != twice {
        return Err(Error::NotIdempotent);
    }
    Ok(once)
}
