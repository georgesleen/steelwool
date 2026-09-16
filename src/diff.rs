//! A dependency free unified diff, used by the `--diff` mode.

use std::fmt::Write;

/// Lines of context kept either side of a change.
const CONTEXT: usize = 3;

/// The largest longest common subsequence table the diff will allocate. Past
/// this the middle section is reported as one wholesale replacement, which is
/// still a correct diff, just a coarse one.
const MAX_CELLS: usize = 4_000_000;

/// Renders the unified diff of `before` against `after`, empty when the two
/// are equal.
pub fn unified(path: &str, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let old = lines(before);
    let new = lines(after);
    let edits = edits(&old, &new);
    let mut out = format!("--- {path}\n+++ {path}\n");
    for hunk in hunks(&edits) {
        render(&mut out, &edits[hunk.start..hunk.end], &hunk);
    }
    out
}

/// Splits into lines without their terminators, ignoring the empty line a
/// trailing newline would otherwise produce.
fn lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit<'a> {
    Keep(&'a str),
    Remove(&'a str),
    Insert(&'a str),
}

impl Edit<'_> {
    fn is_change(&self) -> bool {
        !matches!(self, Edit::Keep(_))
    }
}

/// A run of edits to print, with the line numbers its first line sits at.
struct Hunk {
    start: usize,
    end: usize,
    old_start: usize,
    new_start: usize,
    old_count: usize,
    new_count: usize,
}

fn edits<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Edit<'a>> {
    let prefix = old
        .iter()
        .zip(new.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let rest = old.len().min(new.len()) - prefix;
    let suffix = (0..rest)
        .take_while(|back| {
            old[old.len() - 1 - back] == new[new.len() - 1 - back]
        })
        .count();

    let mut edits: Vec<Edit<'a>> = Vec::with_capacity(old.len() + new.len());
    edits.extend(old[..prefix].iter().map(|line| Edit::Keep(line)));
    let middle_old = &old[prefix..old.len() - suffix];
    let middle_new = &new[prefix..new.len() - suffix];
    middle(middle_old, middle_new, &mut edits);
    edits.extend(
        old[old.len() - suffix..]
            .iter()
            .map(|line| Edit::Keep(line)),
    );
    edits
}

/// Diffs the section left after common prefix and suffix are stripped.
fn middle<'a>(old: &[&'a str], new: &[&'a str], edits: &mut Vec<Edit<'a>>) {
    let (rows, columns) = (old.len(), new.len());
    if rows == 0 || columns == 0 || (rows + 1) * (columns + 1) > MAX_CELLS {
        edits.extend(old.iter().map(|line| Edit::Remove(line)));
        edits.extend(new.iter().map(|line| Edit::Insert(line)));
        return;
    }

    let stride = columns + 1;
    let mut table = vec![0u32; (rows + 1) * stride];
    for row in (0..rows).rev() {
        for column in (0..columns).rev() {
            table[row * stride + column] = if old[row] == new[column] {
                table[(row + 1) * stride + column + 1] + 1
            } else {
                table[(row + 1) * stride + column]
                    .max(table[row * stride + column + 1])
            };
        }
    }

    let (mut row, mut column) = (0, 0);
    while row < rows && column < columns {
        if old[row] == new[column] {
            edits.push(Edit::Keep(old[row]));
            row += 1;
            column += 1;
        } else if table[(row + 1) * stride + column]
            >= table[row * stride + column + 1]
        {
            edits.push(Edit::Remove(old[row]));
            row += 1;
        } else {
            edits.push(Edit::Insert(new[column]));
            column += 1;
        }
    }
    edits.extend(old[row..].iter().map(|line| Edit::Remove(line)));
    edits.extend(new[column..].iter().map(|line| Edit::Insert(line)));
}

/// Groups the changes into hunks, merging any whose context would overlap or
/// touch.
fn hunks(edits: &[Edit<'_>]) -> Vec<Hunk> {
    let changes: Vec<usize> = edits
        .iter()
        .enumerate()
        .filter(|(_, edit)| edit.is_change())
        .map(|(index, _)| index)
        .collect();

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for change in changes {
        let start = change.saturating_sub(CONTEXT);
        let end = (change + CONTEXT + 1).min(edits.len());
        match ranges.last_mut() {
            Some(last) if start <= last.1 => last.1 = end,
            _ => ranges.push((start, end)),
        }
    }

    let mut hunks = Vec::with_capacity(ranges.len());
    let (mut old_line, mut new_line) = (0, 0);
    let mut cursor = 0;
    for (start, end) in ranges {
        for edit in &edits[cursor..start] {
            match edit {
                Edit::Keep(_) => {
                    old_line += 1;
                    new_line += 1;
                }
                Edit::Remove(_) => old_line += 1,
                Edit::Insert(_) => new_line += 1,
            }
        }
        cursor = start;
        let mut old_count = 0;
        let mut new_count = 0;
        for edit in &edits[start..end] {
            match edit {
                Edit::Keep(_) => {
                    old_count += 1;
                    new_count += 1;
                }
                Edit::Remove(_) => old_count += 1,
                Edit::Insert(_) => new_count += 1,
            }
        }
        hunks.push(Hunk {
            start,
            end,
            old_start: start_of(old_line, old_count),
            new_start: start_of(new_line, new_count),
            old_count,
            new_count,
        });
    }
    hunks
}

/// A hunk covering no lines of a side points at the line it follows, as
/// `diff` does; otherwise it points at its own first line.
fn start_of(consumed: usize, count: usize) -> usize {
    if count == 0 { consumed } else { consumed + 1 }
}

fn render(out: &mut String, edits: &[Edit<'_>], hunk: &Hunk) {
    writeln!(
        out,
        "@@ -{},{} +{},{} @@",
        hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
    )
    .expect("writing to a String never fails");
    for edit in edits {
        let (marker, line) = match edit {
            Edit::Keep(line) => (' ', line),
            Edit::Remove(line) => ('-', line),
            Edit::Insert(line) => ('+', line),
        };
        out.push(marker);
        out.push_str(line);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(count: usize) -> String {
        (1..=count).fold(String::new(), |mut text, line| {
            text.push_str(&format!("line {line}\n"));
            text
        })
    }

    #[test]
    fn equal_inputs_diff_to_nothing() {
        assert_eq!(unified("a.scm", "(a)\n", "(a)\n"), "");
        assert_eq!(unified("a.scm", "", ""), "");
    }

    #[test]
    fn one_changed_line_gives_one_hunk_with_three_lines_of_context() {
        let before = numbered(20);
        let after = before.replace("line 10\n", "line ten\n");
        assert_eq!(
            unified("a.scm", &before, &after),
            "--- a.scm\n\
             +++ a.scm\n\
             @@ -7,7 +7,7 @@\n\
             \x20line 7\n\
             \x20line 8\n\
             \x20line 9\n\
             -line 10\n\
             +line ten\n\
             \x20line 11\n\
             \x20line 12\n\
             \x20line 13\n"
        );
    }

    #[test]
    fn far_apart_changes_give_two_hunks() {
        let before = numbered(40);
        let after = before
            .replace("line 5\n", "line five\n")
            .replace("line 30\n", "line thirty\n");
        let diff = unified("a.scm", &before, &after);
        let headers: Vec<&str> =
            diff.lines().filter(|line| line.starts_with("@@")).collect();
        assert_eq!(headers, ["@@ -2,7 +2,7 @@", "@@ -27,7 +27,7 @@"]);
    }

    #[test]
    fn adjacent_changes_give_one_merged_hunk() {
        let before = numbered(20);
        let after = before
            .replace("line 10\n", "line ten\n")
            .replace("line 12\n", "line twelve\n");
        let diff = unified("a.scm", &before, &after);
        let headers: Vec<&str> =
            diff.lines().filter(|line| line.starts_with("@@")).collect();
        assert_eq!(headers, ["@@ -7,9 +7,9 @@"]);
        assert!(diff.contains("-line 10\n+line ten\n line 11\n"), "{diff}");
    }

    #[test]
    fn an_insertion_at_end_of_file_is_a_trailing_hunk() {
        let before = numbered(5);
        let after = format!("{before}line 6\n");
        assert_eq!(
            unified("a.scm", &before, &after),
            "--- a.scm\n\
             +++ a.scm\n\
             @@ -3,3 +3,4 @@\n\
             \x20line 3\n\
             \x20line 4\n\
             \x20line 5\n\
             +line 6\n"
        );
    }

    #[test]
    fn an_insertion_into_an_empty_file_starts_at_zero() {
        assert_eq!(
            unified("a.scm", "", "(a)\n"),
            "--- a.scm\n+++ a.scm\n@@ -0,0 +1,1 @@\n+(a)\n"
        );
    }
}
