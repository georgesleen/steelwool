# Passes

Each pass is a boolean under `[passes]` in the configuration, each defaults to
the pedantic choice, and each is named here by its key. A pass may reorder or
rewrite, so every pass that rewrites datums names the rewrite the equivalence
oracle has to know about (see `correctness.md`).

`golden` means `tests/golden.rs::golden_outputs_match` compares the named input
under `tests/data` against its `.expected` neighbour.

## P1 `provide-one-per-line`

A `provide` with more than one name takes one name per line, regardless of
width. A single name stays on the `provide` line if it fits.

Locked by golden, `provide-require.scm`; off by golden, `passes-off.scm`.

## P2 `sort-require`

Sorts the clauses inside a `require` form, and sorts each run of adjacent top
level `require`, `require-builtin` and `#%require-dylib` forms. A sorted run
carries no blank line between its forms; a blank line before the run survives.

The sort key is the clause rendered on one line, compared bytewise.

Skipped, leaving the form alone, when a clause is the bare symbol `as`, because
`(require-builtin helix/core/text as text.)` is positional, or when a line
comment sits among the clauses, because a comment's position carries meaning.

Rewrite: clause order, oracle rule R1.

Locked by golden, `provide-require.scm`; off by golden, `passes-off.scm`.

## P3 `attach-doc-comments`

Deletes any blank line between a `;;@doc` comment block and the form it
documents. The comment block itself is untouched.

Locked by golden, `doc-comments.scm`; off by golden, `passes-off.scm`.

## P4 `blank-lines`

Exactly one blank line between top level forms, none between the forms of a
`require` run, and none between a leading comment and the form it precedes.
With the pass off, a blank line in the source is preserved and the absence of
one is preserved.

Locked by golden, `blank-lines.scm`; off by golden, `passes-off.scm`.

## P5 `align-let-bindings`

Once a `let` family form has to break, its binding list takes one binding per
line, aligned under the first binding, even where several bindings would fit on
one line.

Locked by golden, `let-bindings.scm`; off by golden, `passes-off.scm`.

## P6 `comment-style`

One comment marker per position, Emacs convention:

- A line comment that starts its own line is written with exactly two
  semicolons.
- A line comment that follows code on its line is written with exactly one
  semicolon.
- The marker is followed by a single space before the body. A body that already
  starts with whitespace keeps it, so indentation inside a comment block
  survives; the pass only ever inserts the first space.
- A comment with an empty body is just its marker.
- A doc comment, one whose body starts with `@doc`, is left byte for byte:
  `;;@doc` reaches the Steel AST as an `@doc` form, so its marker is program
  text rather than punctuation.
- A `#|` block comment is left alone.

Rewrite: comment markers, oracle rule R3.

Locked by golden, `comment-style.scm`; off by golden, `passes-off.scm`.

## P7 `quote-sugar`

Writes the reader shorthand for a two item quoting form:

| Form | Becomes |
| --- | --- |
| `(quote X)` | `'X` |
| `(quasiquote X)` | `` `X `` |
| `(unquote X)` | `,X` |
| `(unquote-splicing X)` | `,@X` |

Applied only when the form holds exactly the head and one datum, holds no
comment, and carries no paren modifier, so `#(quote x)` is left alone. The
shorthand and the long form read as the same datum, including inside a
`syntax-rules` pattern or template.

Rewrite: quoting shorthand, oracle rule R4.

Locked by golden, `quote-sugar.scm`; off by golden, `passes-off.scm`.

## P8 `boolean-spelling`

`#true` becomes `#t` and `#false` becomes `#f`, the spelling the Steel cogs
already prefer. Applied to every boolean literal, including inside quoted data,
where both spellings read as the same datum.

Rewrite: boolean spelling, oracle rule R5.

Locked by golden, `quote-sugar.scm`; off by golden, `passes-off.scm`.

## P9 `sort-provide`

Sorts the names and clauses inside a `provide` form, by the same one line
rendered key `sort-require` uses. `provide` is declarative, so its order carries
no meaning; a `(contract/out ...)` clause sorts by its rendered text like any
other.

Skipped when a line comment sits among the names.

Runs of adjacent top level `provide` forms are left in place: only the inside of
one form is sorted.

Rewrite: clause order, oracle rule R2.

Locked by golden, `provide-require.scm`; off by golden, `passes-off.scm`.

## Ordering

Passes run over the tree bottom up, innermost form first, then the top level
runs. Rewriting passes (P6, P7, P8) run before reordering passes (P2, P9) so a
sort key is computed from the final text of a clause.
