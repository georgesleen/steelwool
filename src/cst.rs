//! Concrete syntax tree built straight from the steel-parser token stream.
//!
//! The Steel AST drops comments, so the tree here is assembled from tokens.

use steel_parser::interner::InternedString;
use steel_parser::lexer::TokenStream;
use steel_parser::tokens::{Paren, ParenMod, TokenType};

use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
    /// The node is the first thing on its source line.
    pub own_line: bool,
    /// At least one blank line preceded the node in the source.
    pub blank_before: bool,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Atom(String),
    Comment(Comment),
    List(List),
    Prefix(Prefix),
    /// A `fmt: off` region, reproduced byte for byte.
    Verbatim(String),
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub text: String,
    /// `#| ... |#` rather than `; ...`.
    pub block: bool,
}

#[derive(Debug, Clone)]
pub struct List {
    pub open: Paren,
    pub modifier: Option<ParenMod>,
    pub items: Vec<Node>,
    /// A pass demanded one item per line regardless of width.
    pub force_break: bool,
}

#[derive(Debug, Clone)]
pub struct Prefix {
    pub text: String,
    pub target: Box<Node>,
}

impl Node {
    pub fn head_name(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::List(list) => {
                list.items.first().and_then(Node::atom_text)
            }
            _ => None,
        }
    }

    pub fn atom_text(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Atom(text) => Some(text),
            _ => None,
        }
    }

    pub fn list_mut(&mut self) -> Option<&mut List> {
        match &mut self.kind {
            NodeKind::List(list) => Some(list),
            _ => None,
        }
    }

    /// A line comment, which swallows the rest of its output line.
    pub fn is_line_comment(&self) -> bool {
        matches!(&self.kind, NodeKind::Comment(comment) if !comment.block)
    }
}

struct Tok<'a> {
    ty: TokenType<InternedString>,
    text: &'a str,
    start: u32,
    end: u32,
    own_line: bool,
    blank_before: bool,
}

/// Lexes and assembles `source`, then folds `fmt: off` regions into verbatim
/// nodes.
pub fn build(source: &str) -> Result<Vec<Node>, Error> {
    let tokens = lex(source)?;
    let nodes = assemble(&tokens)?;
    Ok(fold_protected(source, &tokens, nodes))
}

fn lex(source: &str) -> Result<Vec<Tok<'_>>, Error> {
    let mut tokens = Vec::new();
    let mut previous_end = 0u32;
    for token in TokenStream::new(source, false, None) {
        let token = token.map_err(|error| Error::Lex {
            message: error.ty.to_string(),
            offset: error.span.start(),
        })?;
        let start = token.span.start();
        // A line comment token swallows its terminating newline; keep that
        // newline available so blank lines after a comment are still visible.
        let end = start + token.source.trim_end().len() as u32;
        tokens.push(Tok {
            own_line: starts_line(source, start),
            blank_before: blank_between(source, previous_end, start),
            ty: token.ty,
            text: token.source,
            start,
            end,
        });
        previous_end = end;
    }
    Ok(tokens)
}

fn starts_line(source: &str, offset: u32) -> bool {
    source[..offset as usize]
        .bytes()
        .rev()
        .find(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
        .is_none_or(|byte| byte == b'\n')
}

fn blank_between(source: &str, from: u32, to: u32) -> bool {
    from < to
        && source[from as usize..to as usize]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            >= 2
}

/// A pending quote-like prefix waiting for the datum it applies to.
struct Pending {
    text: String,
    start: u32,
    own_line: bool,
    blank_before: bool,
}

struct Frame {
    open: Paren,
    modifier: Option<ParenMod>,
    start: u32,
    own_line: bool,
    blank_before: bool,
    items: Vec<Node>,
    pending: Vec<Pending>,
}

impl Frame {
    fn root() -> Self {
        Frame {
            open: Paren::Round,
            modifier: None,
            start: 0,
            own_line: true,
            blank_before: false,
            items: Vec::new(),
            pending: Vec::new(),
        }
    }

    /// Wraps `node` in any pending prefixes and files it as an item.
    fn push_datum(&mut self, mut node: Node) {
        while let Some(pending) = self.pending.pop() {
            node = Node {
                span: Span {
                    start: pending.start,
                    end: node.span.end,
                },
                own_line: pending.own_line,
                blank_before: pending.blank_before,
                kind: NodeKind::Prefix(Prefix {
                    text: pending.text,
                    target: Box::new(node),
                }),
            };
        }
        self.items.push(node);
    }
}

fn prefix_text(ty: &TokenType<InternedString>) -> bool {
    matches!(
        ty,
        TokenType::QuoteTick
            | TokenType::QuasiQuote
            | TokenType::Unquote
            | TokenType::UnquoteSplice
            | TokenType::QuoteSyntax
            | TokenType::QuasiQuoteSyntax
            | TokenType::UnquoteSyntax
            | TokenType::UnquoteSpliceSyntax
            | TokenType::DatumComment
    )
}

fn assemble(tokens: &[Tok<'_>]) -> Result<Vec<Node>, Error> {
    let mut stack = vec![Frame::root()];
    for token in tokens {
        let frame = stack.last_mut().expect("root frame is never popped");
        match &token.ty {
            TokenType::OpenParen(open, modifier) => {
                stack.push(Frame {
                    open: *open,
                    modifier: *modifier,
                    start: token.start,
                    own_line: token.own_line,
                    blank_before: token.blank_before,
                    items: Vec::new(),
                    pending: Vec::new(),
                });
            }
            TokenType::CloseParen(close) => {
                if stack.len() == 1 {
                    return Err(Error::UnexpectedClose {
                        offset: token.start,
                    });
                }
                let frame = stack.pop().expect("checked depth above");
                if !frame.pending.is_empty() {
                    return Err(Error::DanglingPrefix {
                        offset: token.start,
                    });
                }
                if frame.open != *close {
                    return Err(Error::MismatchedDelimiter {
                        offset: token.start,
                    });
                }
                let node = Node {
                    kind: NodeKind::List(List {
                        open: frame.open,
                        modifier: frame.modifier,
                        items: frame.items,
                        force_break: false,
                    }),
                    span: Span {
                        start: frame.start,
                        end: token.end,
                    },
                    own_line: frame.own_line,
                    blank_before: frame.blank_before,
                };
                stack
                    .last_mut()
                    .expect("root frame is never popped")
                    .push_datum(node);
            }
            TokenType::Comment => {
                let text = token.text.trim_end().to_string();
                let block = text.starts_with("#|");
                frame.items.push(Node {
                    kind: NodeKind::Comment(Comment { text, block }),
                    span: Span {
                        start: token.start,
                        end: token.end,
                    },
                    own_line: token.own_line,
                    blank_before: token.blank_before,
                });
            }
            ty if prefix_text(ty) => {
                frame.pending.push(Pending {
                    text: token.text.to_string(),
                    start: token.start,
                    own_line: token.own_line,
                    blank_before: token.blank_before,
                });
            }
            _ => {
                let node = Node {
                    kind: NodeKind::Atom(token.text.to_string()),
                    span: Span {
                        start: token.start,
                        end: token.end,
                    },
                    own_line: token.own_line,
                    blank_before: token.blank_before,
                };
                frame.push_datum(node);
            }
        }
    }

    if stack.len() > 1 {
        let frame = stack.last().expect("checked depth above");
        return Err(Error::UnclosedList {
            offset: frame.start,
        });
    }
    let root = stack.pop().expect("root frame is never popped");
    if let Some(pending) = root.pending.first() {
        return Err(Error::DanglingPrefix {
            offset: pending.start,
        });
    }
    Ok(root.items)
}

/// Recognises `;; fmt: off` and `;; fmt: on` markers, any number of leading
/// semicolons.
fn fmt_marker(text: &str) -> Option<bool> {
    let body = text.trim_start_matches(';').trim();
    let rest = body.strip_prefix("fmt:")?.trim();
    match rest {
        "off" => Some(false),
        "on" => Some(true),
        _ => None,
    }
}

fn line_start(source: &str, offset: u32) -> u32 {
    match source[..offset as usize].rfind('\n') {
        Some(index) => index as u32 + 1,
        None => 0,
    }
}

fn line_end(source: &str, offset: u32) -> u32 {
    match source[offset as usize..].find('\n') {
        Some(index) => offset + index as u32,
        None => source.len() as u32,
    }
}

/// Byte ranges the formatter must reproduce untouched.
///
/// A marker inside a form protects the whole enclosing top-level form, so every
/// protected range covers whole top-level forms and whole lines.
fn protected_ranges(
    source: &str,
    tokens: &[Tok<'_>],
    nodes: &[Node],
) -> Vec<(u32, u32)> {
    let enclosing = |offset: u32| -> (u32, u32) {
        nodes
            .iter()
            .find(|node| node.span.start <= offset && offset < node.span.end)
            .map(|node| (node.span.start, node.span.end))
            .unwrap_or((offset, offset))
    };

    let mut ranges = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if token.ty != TokenType::Comment
            || fmt_marker(token.text) != Some(false)
        {
            index += 1;
            continue;
        }
        let (form_start, _) = enclosing(token.start);
        let start = line_start(source, form_start.min(token.start));

        let mut end = source.len() as u32;
        let mut cursor = index + 1;
        while cursor < tokens.len() {
            let candidate = &tokens[cursor];
            if candidate.ty == TokenType::Comment
                && fmt_marker(candidate.text) == Some(true)
            {
                let (_, form_end) = enclosing(candidate.start);
                end = line_end(source, form_end.max(candidate.end));
                break;
            }
            cursor += 1;
        }
        ranges.push((start, end));
        index = cursor + 1;
    }
    ranges
}

fn fold_protected(
    source: &str,
    tokens: &[Tok<'_>],
    nodes: Vec<Node>,
) -> Vec<Node> {
    let ranges = protected_ranges(source, tokens, &nodes);
    if ranges.is_empty() {
        return nodes;
    }

    let mut folded: Vec<Node> = Vec::with_capacity(nodes.len());
    let mut consumed: Vec<bool> = vec![false; ranges.len()];
    for node in nodes {
        let hit = ranges.iter().position(|(start, end)| {
            node.span.start < *end && node.span.end > *start
        });
        match hit {
            Some(index) => {
                if consumed[index] {
                    continue;
                }
                consumed[index] = true;
                let (start, end) = ranges[index];
                folded.push(Node {
                    kind: NodeKind::Verbatim(
                        source[start as usize..end as usize]
                            .trim_end()
                            .to_string(),
                    ),
                    span: Span { start, end },
                    own_line: true,
                    blank_before: node.blank_before,
                });
            }
            None => folded.push(node),
        }
    }
    folded
}
