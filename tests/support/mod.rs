//! Helpers shared by the golden suite and the property suite.

use steel_parser::lexer::TokenStream;
use steel_parser::tokens::TokenType;

/// Lines carrying a token the formatter may not touch: comment text, which is
/// never rewrapped, and any token that spans lines of its own, a string
/// literal or a block comment.
fn untouchable_lines(formatted: &str) -> Vec<bool> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(formatted.match_indices('\n').map(|(index, _)| index + 1))
        .collect();
    let line_of = |offset: usize| match starts.binary_search(&offset) {
        Ok(line) => line,
        Err(next) => next - 1,
    };
    let mut untouchable = vec![false; starts.len()];
    for token in TokenStream::new(formatted, false, None) {
        let token = token.expect("formatted output lexes");
        let first = line_of(token.span.start() as usize);
        let last = line_of(token.span.end().saturating_sub(1) as usize);
        let excused = matches!(
            token.ty,
            TokenType::Comment | TokenType::StringLiteral(_)
        ) || last > first;
        for line in first..=last.min(untouchable.len() - 1) {
            untouchable[line] |= excused;
        }
    }
    untouchable
}

/// The lines of `formatted` that overrun `width` without an excuse, as
/// `(line number, line)` pairs with the line number one based.
///
/// A line is excused when it carries a token the formatter may not touch, or
/// when it holds no interior whitespace: steelwool separates every pair of
/// items by whitespace, so such a line is a single item with its delimiters
/// and no layout choice could have shortened it.
pub fn unexcused_overruns(
    formatted: &str,
    width: usize,
) -> Vec<(usize, String)> {
    let untouchable = untouchable_lines(formatted);
    formatted
        .lines()
        .enumerate()
        .filter(|(number, line)| {
            line.chars().count() > width
                && !untouchable[*number]
                && line.trim_start().contains(' ')
        })
        .map(|(number, line)| (number + 1, line.to_string()))
        .collect()
}
