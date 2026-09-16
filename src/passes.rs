//! Named passes, each a boolean in `[passes]`, each defaulting to on.

use steel_parser::tokens::Paren;

use crate::config::Config;
use crate::cst::{Comment, List, Node, NodeKind, Prefix};
use crate::layout;

pub fn apply(nodes: &mut [Node], config: &Config) {
    for node in nodes.iter_mut() {
        walk(node, config);
    }
    if config.passes.sort_require {
        sort_require_block(nodes);
    }
}

/// Rewrites run innermost first, so that a sort key is taken from the final
/// text of a clause.
fn walk(node: &mut Node, config: &Config) {
    let own_line = node.own_line;
    match &mut node.kind {
        NodeKind::Atom(text) => {
            if config.passes.boolean_spelling {
                boolean_spelling(text);
            }
        }
        NodeKind::Comment(comment) => {
            if config.passes.comment_style {
                comment_style(comment, own_line);
            }
        }
        NodeKind::Prefix(prefix) => walk(&mut prefix.target, config),
        NodeKind::Verbatim(_) => {}
        NodeKind::List(list) => {
            for item in list.items.iter_mut() {
                walk(item, config);
            }
            head_passes(list, config);
        }
    }
    if config.passes.quote_sugar {
        quote_sugar(node);
    }
}

/// The passes that a form's head selects.
fn head_passes(list: &mut List, config: &Config) {
    let head = list.items.first().and_then(Node::atom_text);
    let sort = match head {
        Some("require") => config.passes.sort_require,
        Some("provide") => config.passes.sort_provide,
        _ => false,
    };
    let one_per_line =
        head == Some("provide") && config.passes.provide_one_per_line;
    if sort {
        sort_clauses(list);
    }
    if one_per_line {
        list.force_break = list.items.len() > 2;
    }
}

/// Sorting is only safe while every clause is self contained, so a positional
/// `as` rename or a comment among the clauses disables it.
fn sort_clauses(list: &mut List) {
    if list.items.len() < 3 {
        return;
    }
    let positional_or_commented = list.items[1..]
        .iter()
        .any(|item| item.atom_text() == Some("as") || item.is_comment());
    if positional_or_commented {
        return;
    }
    list.items[1..].sort_by_cached_key(flat_key);
}

/// Sorts each run of adjacent top-level require forms.
fn sort_require_block(nodes: &mut [Node]) {
    let mut start = 0;
    while start < nodes.len() {
        if !layout::require_family(&nodes[start]) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < nodes.len() && layout::require_family(&nodes[end]) {
            end += 1;
        }
        if end - start > 1 {
            let leading_blank = nodes[start].blank_before;
            nodes[start..end].sort_by_cached_key(flat_key);
            for node in nodes[start..end].iter_mut() {
                node.blank_before = false;
            }
            nodes[start].blank_before = leading_blank;
        }
        start = end;
    }
}

/// Two semicolons on a comment that owns its line, one on a comment that
/// follows code. A doc comment reaches the Steel AST as an `@doc` form, so its
/// marker is program text and is left alone.
fn comment_style(comment: &mut Comment, own_line: bool) {
    if comment.block || layout::is_doc_marker(&comment.text) {
        return;
    }
    let marker = if own_line { ";;" } else { ";" };
    let body = comment.text.trim_start_matches(';');
    // Whitespace the body already carries is indentation within a comment
    // block, so the pass only ever inserts the first space.
    let text = if body.is_empty() {
        marker.to_string()
    } else if body.starts_with([' ', '\t']) {
        format!("{marker}{body}")
    } else {
        format!("{marker} {body}")
    };
    comment.text = text;
}

fn boolean_spelling(text: &mut String) {
    let short = match text.as_str() {
        "#true" => "#t",
        "#false" => "#f",
        _ => return,
    };
    text.clear();
    text.push_str(short);
}

/// A two item quoting form becomes its reader shorthand, which reads as the
/// same datum.
fn quote_sugar(node: &mut Node) {
    let NodeKind::List(list) = &mut node.kind else {
        return;
    };
    if list.open != Paren::Round
        || list.modifier.is_some()
        || list.items.len() != 2
        || list.items.iter().any(Node::is_comment)
    {
        return;
    }
    let Some(marker) = list
        .items
        .first()
        .and_then(Node::atom_text)
        .and_then(shorthand)
    else {
        return;
    };
    let target = list.items.pop().expect("the form has two items");
    node.kind = NodeKind::Prefix(Prefix {
        text: marker.to_string(),
        target: Box::new(target),
    });
}

fn shorthand(head: &str) -> Option<&'static str> {
    Some(match head {
        "quote" => "'",
        "quasiquote" => "`",
        "unquote" => ",",
        "unquote-splicing" => ",@",
        _ => return None,
    })
}

/// A stable sort key: the node rendered on one line.
fn flat_key(node: &Node) -> String {
    let mut out = String::new();
    write_flat(&mut out, node);
    out
}

fn write_flat(out: &mut String, node: &Node) {
    match &node.kind {
        NodeKind::Atom(text) => out.push_str(text),
        NodeKind::Verbatim(text) => out.push_str(text),
        NodeKind::Comment(comment) => out.push_str(&comment.text),
        NodeKind::Prefix(prefix) => {
            out.push_str(&prefix.text);
            write_flat(out, &prefix.target);
        }
        NodeKind::List(list) => {
            out.push(list.open.open());
            for (index, item) in list.items.iter().enumerate() {
                if index > 0 {
                    out.push(' ');
                }
                write_flat(out, item);
            }
            out.push(list.open.close());
        }
    }
}
