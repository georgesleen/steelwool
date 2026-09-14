//! Named passes, each a boolean in `[passes]`, each defaulting to on.

use crate::config::Config;
use crate::cst::{Node, NodeKind};
use crate::layout;

pub fn apply(nodes: &mut [Node], config: &Config) {
    for node in nodes.iter_mut() {
        walk(node, config);
    }
    if config.passes.sort_require {
        sort_require_block(nodes);
    }
}

fn walk(node: &mut Node, config: &Config) {
    let Some(list) = node.list_mut() else {
        return;
    };
    for item in list.items.iter_mut() {
        walk(item, config);
    }

    let head = list.items.first().and_then(Node::atom_text);
    match head {
        Some("provide") if config.passes.provide_one_per_line => {
            list.force_break = list.items.len() > 2;
        }
        Some("require") if config.passes.sort_require => {
            sort_clauses(node);
        }
        _ => {}
    }
}

/// Sorting is only safe while every clause is self contained, so a positional
/// `as` rename disables it.
fn sort_clauses(node: &mut Node) {
    let Some(list) = node.list_mut() else {
        return;
    };
    if list.items.len() < 3 {
        return;
    }
    if list.items[1..]
        .iter()
        .any(|item| item.atom_text() == Some("as"))
    {
        return;
    }
    if list.items[1..].iter().any(Node::is_line_comment) {
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
