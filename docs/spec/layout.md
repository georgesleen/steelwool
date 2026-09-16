# Layout

Normative rules for how steelwool arranges text. Rule ids are stable; each rule
names the test that locks it. `width` is the only knob these rules read.

`golden` below means `tests/golden.rs::golden_outputs_match` compares the named
input under `tests/data` against its `.expected` neighbour.

## Whitespace

| Rule | Statement | Locked by |
| --- | --- | --- |
| L1 | Indentation is two spaces per level of nesting. It is not configurable. | golden, every input |
| L2 | A run of whitespace between two items on one line is a single space. | golden, `comments.scm` |
| L3 | Output ends with exactly one newline. Empty input produces empty output. | `golden.rs::output_ends_with_one_newline` |
| L4 | No line ends in whitespace. | `golden.rs::no_line_ends_in_whitespace` |
| L5 | No tab character appears outside a string literal or comment text. | `golden.rs::no_tabs_outside_strings_and_comments` |

## Breaking

| Rule | Statement | Locked by |
| --- | --- | --- |
| L6 | A form is written on one line when it fits: its own width plus the column it starts at plus the closing delimiters its ancestors still owe that line is at most `width`. | golden, `wrap-at-width.scm` |
| L7 | A form that does not fit breaks. Its items are then one per line at the plan indent, except as L11 and L12 allow. | golden, `wrap-at-width.scm` |
| L8 | A call form prefers a hanging indent aligned one column past its head, and falls back to a two space body indent when the aligned plan overruns `width`. | golden, `narrow-width.scm` |
| L9 | A special form keeps a fixed number of items on its opening line and indents the rest two spaces: 0 for `begin`, `cond`, `case-lambda`; 1 for `define`, `define-values`, `define-syntax`, `define-record-type`, `struct`, `lambda`, `λ`, `fn`, `when`, `unless`, `while`, `syntax-rules`, `case`, `match`, `module`; 2 for `do`. | golden, `nested-special-forms.scm` |
| L10 | A `let` family head (`let`, `let*`, `letrec`, `letrec*`, `let-values`, `let*-values`, `let-syntax`, `letrec-syntax`) or `parameterize` keeps its binding list on the opening line, after the loop name when the `let` is named. | golden, `let-bindings.scm` |
| L11 | Among candidate plans steelwool takes the first that overruns `width` nowhere, otherwise the one whose total overrun is smallest. | golden, `narrow-width.scm` |
| L12 | A blank line between two items of a form is preserved as exactly one blank line. | golden, `blank-lines.scm` |

## Comments

| Rule | Statement | Locked by |
| --- | --- | --- |
| L13 | Comment text is never rewrapped, never merged with code, and never reflowed across lines. | `golden.rs::formatting_preserves_the_program` |
| L14 | A comment that began its source line keeps its own line, and forces the form containing it to break. | golden, `comments.scm` |
| L15 | A comment that followed code on its source line stays on the line of the item it followed. | golden, `comments.scm` |
| L16 | A `#\|` block comment is reproduced verbatim, including its internal newlines. | golden, `comments.scm` |

## Width overruns

| Rule | Statement | Locked by |
| --- | --- | --- |
| L17 | A line may exceed `width` only when it carries comment text, ends in a trailing comment, or holds a single atom that cannot be split. Every other line is at most `width` columns. | `golden.rs::code_lines_respect_the_width` |
| L18 | Width is counted in characters, not bytes. | `golden.rs::code_lines_respect_the_width` |

## Protected regions

| Rule | Statement | Locked by |
| --- | --- | --- |
| L19 | The region from a `;; fmt: off` marker to the matching `;; fmt: on` is reproduced byte for byte. Any number of leading semicolons marks. | `golden.rs::fmt_off_regions_survive_unchanged` |
| L20 | A marker inside a form protects the whole enclosing top level form, so a protected region always covers whole forms and whole lines. | golden, `fmt-off.scm` |
| L21 | An unmatched `;; fmt: off` protects the rest of the file. | golden, `fmt-off.scm` |
