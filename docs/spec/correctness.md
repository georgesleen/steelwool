# Correctness

Formatting must never change the program. These rules say what "the same
program" means, and what the formatter is allowed to change inside it.

## The oracle

| Rule | Statement | Locked by |
| --- | --- | --- |
| V1 | `verify::check_equivalence` lexes input and output into span free datum trees and compares them after canonicalization. A comment is a datum in that tree: `;;@doc` reaches the Steel AST as an `@doc` form, so losing or rewording a comment can change the program. | `golden.rs::the_equivalence_check_rejects_a_changed_program` |
| V2 | The Steel parser must still accept the output and still see the same number of top level forms. Input the parser rejects or panics on is exempt because steelwool formats what it can lex. The pinned parser's long-form quasiquote context defect is the one narrower exemption documented under `README.md#workarounds`. | `golden.rs::the_equivalence_check_rejects_a_changed_program`, `property.rs::arbitrary_text_never_panics` |
| V3 | `format_source` runs the oracle on every call and returns `Error::ProgramChanged` rather than emitting output that fails it. | `golden.rs::formatting_preserves_the_program` |
| V4 | `verify::check_idempotence` formats twice and compares. Every input in the suite, including the ten cogs under `tests/corpus`, is a fixed point after one pass. | `golden.rs::formatting_is_idempotent` |

## Allowed rewrites

Canonicalization is applied to both sides, so each rule below is deliberately a
blind spot in the oracle. A rewrite earns an entry only when it is a reader
level identity, and the pass that performs it is pinned by golden data instead.

| Rule | Canonicalization | For |
| --- | --- | --- |
| R1 | Clauses of a `require` form are sorted, and runs of adjacent top level require family forms are sorted. Skipped where a clause is `as`, matching the pass. | P2 `sort-require` |
| R2 | Clauses of a `provide` form are sorted. | P9 `sort-provide` |
| R3 | A line comment canonicalizes to its body with leading semicolons and surrounding whitespace stripped. A doc comment, body starting `@doc`, is exempt and compares byte for byte, marker included. | P6 `comment-style` |
| R4 | A quote prefix folds onto the datum that follows it, and `'X`, `` `X ``, `,X`, `,@X` canonicalize to `(quote X)`, `(quasiquote X)`, `(unquote X)`, `(unquote-splicing X)`. | P7 `quote-sugar` |
| R5 | `#true` canonicalizes to `#t` and `#false` to `#f`. | P8 `boolean-spelling` |

R3 costs the oracle its sight of the exact marker on a non doc comment. R4
costs it the distinction between shorthand and long form. R5 costs it the
spelling of a boolean. Nothing else is canonicalized: a dropped comment, a
reordered argument, a changed atom, a lost form are all still caught.

## Properties

`tests/property.rs` generates Steel source from a datum tree strategy, with
arbitrary indentation, blank lines, comments and quoting, and asserts:

| Rule | Statement |
| --- | --- |
| V5 | Formatting generated source succeeds, so no generated input trips the oracle. |
| V6 | Formatting is idempotent on generated source. |
| V7 | Every line of the output respects `width`, at the width the case was generated for, except the overruns layout rule L17 permits. |
| V8 | Formatting arbitrary text never panics: it returns output or an error. |

Failing property cases are shrunk and recorded in
`tests/property.proptest-regressions`; when created, that file is checked in
and replayed on every run.

## Why not the AST

`ExprKind`'s derived `PartialEq` is unusable as an equivalence oracle.
`List::syntax_object_id`, `LambdaFunction::syntax_object_id` and
`List::location` all take part in it, and the ids are freshly allocated per
parse, so two parses of the same text compare unequal. Hence the datum tree
comparison in V1 rather than `before == after` on the AST.
