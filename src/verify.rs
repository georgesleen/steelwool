//! The invariant that matters: formatting must never change the program.

use steel_parser::lexer::TokenStream;
use steel_parser::parser::Parser;
use steel_parser::tokens::TokenType;

use crate::config::Config;
use crate::layout::REQUIRE_HEADS;
use crate::{Error, format_source};

/// A span free, comment free datum tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Sexp {
    Atom(String),
    List(char, Vec<Sexp>),
}

/// Lexes `source` into datum trees. Comments are datums here: a `;;@doc` block
/// becomes an `@doc` form in the Steel AST, so losing or rewording a comment
/// can change the program.
fn datums(source: &str) -> Result<Vec<Sexp>, Error> {
    let mut stack: Vec<(char, Vec<Sexp>)> = vec![('\0', Vec::new())];
    for token in TokenStream::new(source, false, None) {
        let token = token.map_err(|error| Error::Lex {
            message: error.ty.to_string(),
            offset: error.span.start(),
        })?;
        match &token.ty {
            TokenType::OpenParen(open, _) => {
                stack.push((open.close(), Vec::new()));
            }
            TokenType::CloseParen(close) => {
                if stack.len() == 1 {
                    return Err(Error::UnexpectedClose {
                        offset: token.span.start(),
                    });
                }
                let (expected, items) =
                    stack.pop().expect("checked depth above");
                if expected != close.close() {
                    return Err(Error::MismatchedDelimiter {
                        offset: token.span.start(),
                    });
                }
                stack
                    .last_mut()
                    .expect("root frame is never popped")
                    .1
                    .push(Sexp::List(expected, items));
            }
            _ => stack
                .last_mut()
                .expect("root frame is never popped")
                .1
                .push(Sexp::Atom(token.source.trim_end().to_string())),
        }
    }
    if stack.len() != 1 {
        return Err(Error::UnclosedList { offset: 0 });
    }
    Ok(stack.pop().expect("root frame is never popped").1)
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

/// Applies the reorderings the formatter is allowed to perform, so that the
/// comparison ignores them on both sides.
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
    let Sexp::List(_, items) = form else {
        return;
    };
    for item in items.iter_mut() {
        canonicalize_nested(item);
    }
    if items.len() < 3
        || items.first() != Some(&Sexp::Atom("require".to_string()))
    {
        return;
    }
    let positional_or_commented = items[1..].iter().any(|item| {
        matches!(item, Sexp::Atom(text) if text == "as" || text.starts_with(';'))
    });
    if positional_or_commented {
        return;
    }
    items[1..].sort();
}

/// Fails when the formatted output is not the same program as the input.
pub fn check_equivalence(before: &str, after: &str) -> Result<(), Error> {
    let mut original = datums(before)?;
    let mut formatted = datums(after)?;
    canonicalize(&mut original);
    canonicalize(&mut formatted);
    if original != formatted {
        return Err(Error::ProgramChanged);
    }

    let before_forms = Parser::new(before, None).collect::<Result<Vec<_>, _>>();
    let Ok(before_forms) = before_forms else {
        return Ok(());
    };
    match Parser::new(after, None).collect::<Result<Vec<_>, _>>() {
        Ok(after_forms) if after_forms.len() == before_forms.len() => Ok(()),
        Ok(after_forms) => Err(Error::OutputRejected(format!(
            "{} top level forms became {}",
            before_forms.len(),
            after_forms.len()
        ))),
        Err(error) => Err(Error::OutputRejected(error.to_string())),
    }
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
