//! The layout engine. Two space indentation, hanging indents, one width knob.

use crate::config::Config;
use crate::cst::{List, Node, NodeKind};

pub const REQUIRE_HEADS: [&str; 3] =
    ["require", "require-builtin", "#%require-dylib"];

const LET_HEADS: [&str; 8] = [
    "let",
    "let*",
    "letrec",
    "letrec*",
    "let-values",
    "let*-values",
    "let-syntax",
    "letrec-syntax",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Normal,
    Bindings,
}

/// Items after the head that share the opening line of a broken form.
fn special_form(name: &str) -> Option<usize> {
    Some(match name {
        "begin" | "cond" | "case-lambda" => 0,
        "define" | "define-values" | "define-syntax" | "define-record-type"
        | "struct" | "lambda" | "λ" | "fn" | "when" | "unless" | "while"
        | "syntax-rules" | "case" | "match" | "module" => 1,
        "do" => 2,
        _ => return None,
    })
}

fn is_let_family(name: &str) -> bool {
    LET_HEADS.contains(&name)
}

/// The index of the binding list, accounting for a named `let`.
fn bindings_index(list: &List, name: &str) -> Option<usize> {
    if is_let_family(name) {
        return match list.items.get(1) {
            Some(node) if node.atom_text().is_some() => Some(2),
            Some(_) => Some(1),
            None => None,
        };
    }
    if name == "parameterize" {
        return Some(1);
    }
    None
}

pub fn require_family(node: &Node) -> bool {
    node.head_name()
        .is_some_and(|name| REQUIRE_HEADS.contains(&name))
}

/// Recognises the `;;@doc` marker that opens a documentation comment block.
pub fn is_doc_marker(text: &str) -> bool {
    text.trim_start_matches(';')
        .trim_start()
        .starts_with("@doc")
}

fn width_of(text: &str) -> usize {
    text.chars().count()
}

fn open_delimiter(list: &List) -> String {
    match list.modifier {
        Some(modifier) => format!("{modifier}{}", list.open.open()),
        None => list.open.open().to_string(),
    }
}

struct Writer {
    out: String,
    col: usize,
    /// Closing delimiters the enclosing forms will append to the current line.
    reserved: usize,
}

impl Writer {
    fn new() -> Self {
        Writer {
            out: String::new(),
            col: 0,
            reserved: 0,
        }
    }

    fn push(&mut self, text: &str) {
        for character in text.chars() {
            if character == '\n' {
                self.col = 0;
            } else {
                self.col += 1;
            }
        }
        self.out.push_str(text);
    }

    fn push_char(&mut self, character: char) {
        self.col += 1;
        self.out.push(character);
    }

    fn newline(&mut self, indent: usize) {
        self.out.push('\n');
        self.indent(indent);
    }

    fn blank_line(&mut self, indent: usize) {
        self.out.push_str("\n\n");
        self.indent(indent);
    }

    fn indent(&mut self, indent: usize) {
        self.out.extend(std::iter::repeat_n(' ', indent));
        self.col = indent;
    }
}

/// The single line rendering of a node, or `None` when it must break.
fn flat(node: &Node) -> Option<String> {
    match &node.kind {
        NodeKind::Atom(text) => Some(text.clone()),
        NodeKind::Verbatim(_) => None,
        NodeKind::Comment(comment) => (comment.block
            && !comment.text.contains('\n'))
        .then(|| comment.text.clone()),
        NodeKind::Prefix(prefix) => {
            Some(format!("{}{}", prefix.text, flat(&prefix.target)?))
        }
        NodeKind::List(list) => flat_list(list),
    }
}

fn flat_list(list: &List) -> Option<String> {
    if list.force_break {
        return None;
    }
    let mut out = open_delimiter(list);
    for (index, item) in list.items.iter().enumerate() {
        if index > 0 {
            if item.blank_before {
                return None;
            }
            out.push(' ');
        }
        out.push_str(&flat(item)?);
    }
    out.push(list.open.close());
    Some(out)
}

pub fn render(nodes: &[Node], config: &Config) -> String {
    let mut writer = Writer::new();
    let mut doc_run = false;
    for (index, node) in nodes.iter().enumerate() {
        if index > 0 {
            match separator(&nodes[index - 1], node, doc_run, config) {
                Separator::Space => writer.push(" "),
                Separator::Line => writer.newline(0),
                Separator::Blank => writer.blank_line(0),
            }
        }
        render_node(&mut writer, node, Role::Normal, config);
        doc_run = match &node.kind {
            NodeKind::Comment(comment) => {
                let marker = is_doc_marker(&comment.text);
                if node.blank_before {
                    marker
                } else {
                    doc_run || marker
                }
            }
            _ => false,
        };
    }
    let mut out = writer.out;
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

enum Separator {
    Space,
    Line,
    Blank,
}

fn separator(
    previous: &Node,
    current: &Node,
    doc_run: bool,
    config: &Config,
) -> Separator {
    let current_is_comment = matches!(current.kind, NodeKind::Comment(_));
    if current_is_comment && !current.own_line {
        return Separator::Space;
    }
    if matches!(previous.kind, NodeKind::Comment(_)) {
        if !current.blank_before {
            return Separator::Line;
        }
        if doc_run && config.passes.attach_doc_comments && !current_is_comment {
            return Separator::Line;
        }
    }
    if config.passes.blank_lines {
        if require_family(previous) && require_family(current) {
            return Separator::Line;
        }
        return Separator::Blank;
    }
    if current.blank_before {
        Separator::Blank
    } else {
        Separator::Line
    }
}

fn render_node(writer: &mut Writer, node: &Node, role: Role, config: &Config) {
    match &node.kind {
        NodeKind::Atom(text) => writer.push(text),
        NodeKind::Comment(comment) => writer.push(&comment.text),
        NodeKind::Verbatim(text) => writer.push(text),
        NodeKind::Prefix(prefix) => {
            writer.push(&prefix.text);
            render_node(writer, &prefix.target, Role::Normal, config);
        }
        NodeKind::List(list) => render_list(writer, list, role, config),
    }
}

/// A candidate layout: how many items share the opening line, and the indent
/// for everything after them.
#[derive(Debug, Clone, Copy)]
struct Plan {
    line_items: usize,
    indent: usize,
}

/// Candidate layouts in preference order. A hanging indent aligned with the
/// first argument is preferred, with the two space body indent as the fallback
/// for when that pushes arguments past the width.
fn plans(list: &List, config: &Config, open_col: usize) -> Vec<Plan> {
    let head = match list.items.first() {
        Some(head) => head,
        None => {
            return vec![Plan {
                line_items: 1,
                indent: open_col + 1,
            }];
        }
    };
    let body = Plan {
        line_items: 1,
        indent: open_col + 2,
    };
    if let Some(name) = head.atom_text() {
        if let Some(index) = bindings_index(list, name) {
            return vec![Plan {
                line_items: 1 + index,
                indent: open_col + 2,
            }];
        }
        if let Some(distinguished) = special_form(name) {
            return vec![Plan {
                line_items: 1 + distinguished,
                indent: open_col + 2,
            }];
        }
        let aligned = open_col + 1 + width_of(name) + 1;
        if aligned < config.width {
            return vec![
                Plan {
                    line_items: 2,
                    indent: aligned,
                },
                body,
            ];
        }
        return vec![body];
    }
    vec![Plan {
        line_items: 1,
        indent: open_col + 1,
    }]
}

fn child_role(list: &List, index: usize) -> Role {
    let name = match list.items.first().and_then(Node::atom_text) {
        Some(name) => name,
        None => return Role::Normal,
    };
    if bindings_index(list, name) == Some(index) {
        Role::Bindings
    } else {
        Role::Normal
    }
}

fn render_list(writer: &mut Writer, list: &List, role: Role, config: &Config) {
    let open_col = writer.col;
    // An aligned binding list takes one binding per line once its `let` broke.
    let one_per_line =
        role == Role::Bindings && config.passes.align_let_bindings;
    if !one_per_line
        && let Some(text) = flat_list(list)
        && open_col + width_of(&text) + writer.reserved <= config.width
    {
        writer.push(&text);
        return;
    }

    let candidates = plans(list, config, open_col);
    let mark = writer.out.len();
    let mut best: Option<(usize, String)> = None;
    for candidate in &candidates {
        writer.out.truncate(mark);
        writer.col = open_col;
        emit(writer, list, *candidate, config);
        let cost = overflow(
            &writer.out[mark..],
            open_col,
            writer.reserved,
            config.width,
        );
        if cost == 0 {
            return;
        }
        if best.as_ref().is_none_or(|(lowest, _)| cost < *lowest) {
            best = Some((cost, writer.out[mark..].to_string()));
        }
    }
    if let Some((_, text)) = best {
        writer.out.truncate(mark);
        writer.col = open_col;
        writer.push(&text);
    }
}

/// Total number of columns by which the region overruns `width`, counting the
/// closing delimiters the enclosing forms still owe the last line.
fn overflow(
    region: &str,
    start_col: usize,
    reserved: usize,
    width: usize,
) -> usize {
    let mut total = 0;
    let last = region.split('\n').count().saturating_sub(1);
    for (index, line) in region.split('\n').enumerate() {
        let mut column = width_of(line);
        if index == 0 {
            column += start_col;
        }
        if index == last {
            column += reserved;
        }
        total += column.saturating_sub(width);
    }
    total
}

fn emit(writer: &mut Writer, list: &List, plan: Plan, config: &Config) {
    writer.push(&open_delimiter(list));
    let close = list.open.close();
    if list.items.is_empty() {
        writer.push_char(close);
        return;
    }

    let mut broken_by_comment = false;
    let mut ends_in_line_comment = false;
    for (index, item) in list.items.iter().enumerate() {
        let own_line_comment = item.is_line_comment() && item.own_line;
        let trailing_comment = item.is_line_comment() && !item.own_line;
        let forced = item.blank_before || own_line_comment;
        let inline = index < plan.line_items && !broken_by_comment && !forced;
        let same_line = (trailing_comment && !ends_in_line_comment) || inline;
        if index == 0 {
            if own_line_comment {
                writer.newline(plan.indent);
            }
        } else if same_line {
            writer.push(" ");
        } else if item.blank_before {
            writer.blank_line(plan.indent);
        } else {
            writer.newline(plan.indent);
        }
        if index + 1 == list.items.len() {
            writer.reserved += 1;
            render_node(writer, item, child_role(list, index), config);
            writer.reserved -= 1;
        } else {
            render_node(writer, item, child_role(list, index), config);
        }
        ends_in_line_comment = item.is_line_comment();
        broken_by_comment |= ends_in_line_comment;
    }
    if ends_in_line_comment {
        writer.newline(plan.indent);
    }
    writer.push_char(close);
}
