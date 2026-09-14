# steelwool

An opinionated formatter for [Steel](https://github.com/mattwparas/steel), the
Scheme dialect embedded in the Steel fork of Helix. Modelled on
[pedantix](https://github.com/Swarsel/pedantix): opinionated in its defaults
rather than by withholding options.

Reads files in place, or stdin to stdout. There is one layout knob and a named
boolean per pass.

## Status

Pre-1.0. Every release stays below `1.0.0` until the command line surface and
the configuration keys are stable, and a breaking change may land in any `0.y`
bump. Pin an exact version if that matters to you.

### Compatibility

steelwool parses with `steel-parser` pinned by git revision, because the crate
published to crates.io lags the fork Helix uses:

```toml
steel-parser = { git = "https://github.com/mattwparas/steel.git", rev = "1b785a4e9d24e3553b242522b35d4498dae72816" }
```

That revision is the one the `steel-event-system` branch of
`mattwparas/helix` pins, so steelwool accepts exactly the syntax that fork's
engine accepts. Bumping it is the change most likely to break consumers, so it
gets a changelog entry of its own every time.

## Install and use

```sh
nix develop          # or direnv allow
make build
```

```sh
steelwool src/*.scm                  # rewrite in place
steelwool                            # stdin to stdout
echo '(define(f x)(+ x 1))' | steelwool -
steelwool --check src/*.scm          # write nothing, exit 1 if anything would change
```

| Option | Effect |
| --- | --- |
| `FILES...` | Format each file in place. |
| none, or `-` | Read stdin, write stdout. |
| `--check` | Write nothing. Exit 1 if any input would change. |
| `--config PATH` | Use `PATH` instead of searching for a config file. |
| `--config-toml TOML` | Apply a TOML string as a configuration layer. |
| `--set KEY.PATH=VALUE` | Override one dotted key with a TOML value. Repeatable. |

Exit codes: `0` clean, `1` something would change under `--check`, `2` error.

Layers apply lowest to highest: built-in defaults, then the config file, then
`--config-toml`, then each `--set` in order.

### Configuration discovery

For each input, steelwool looks for `steelwool.toml` then `.steelwool.toml` in
the file's directory and each ancestor, stopping after the directory that
contains `.git`. Failing that, it reads
`$XDG_CONFIG_HOME/steelwool/steelwool.toml`. `--config` skips the search.

## Configuration

Opinionated defaults, few knobs. Every key below is the complete set; an
unrecognised key is a hard error rather than a silent no-op.

```toml
# The only layout knob.
width = 80

[passes]
provide-one-per-line = true
sort-require = true
attach-doc-comments = true
blank-lines = true
align-let-bindings = true
```

Indentation is fixed at two spaces, with hanging indents under special forms,
and is not configurable.

| Key | Pedantic default does this |
| --- | --- |
| `width` | Wraps code at 80 columns. |
| `provide-one-per-line` | A `provide` with more than one name takes one name per line. |
| `sort-require` | Sorts the clauses inside a `require` and the forms inside a run of adjacent top-level `require` forms. |
| `attach-doc-comments` | Deletes any blank line between a `;;@doc` comment block and the form it documents. |
| `blank-lines` | Exactly one blank line between top-level forms, none inside a require block, none between a leading comment and its form. |
| `align-let-bindings` | Once a `let` has to break, its bindings take one line each, aligned under the first binding. |

`sort-require` leaves a `require` alone when a clause is the bare symbol `as`,
since `(require-builtin helix/core/text as text.)` is positional.

## Escape hatch

```scheme
;; fmt: off
(define matrix
  '(( 1  0  0 )
    ( 0  1  0 )
    ( 0  0  1 )))
;; fmt: on
```

The region between the markers is reproduced byte for byte. A marker inside a
form protects the whole enclosing top-level form, so a protected region always
covers whole forms and whole lines. An unmatched `;; fmt: off` protects the rest
of the file.

## Correctness

Formatting must never change the program. Two checks enforce that:

- **Parse equivalence.** `steelwool::verify::check_equivalence` lexes the input
  and the output into span-free, comment-free datum trees and compares them,
  after applying the reorderings the passes are allowed to perform. It also
  requires that the Steel parser still accepts the output and still sees the
  same number of top-level forms. `format_source` runs this on every call and
  returns an error rather than emitting output that fails it.
- **Idempotence.** `steelwool::verify::check_idempotence` formats twice and
  compares.

The test suite runs both over every input under `tests/data` and over
`tests/corpus`, which holds the ten Steel cogs shipped inside the Helix fork.

Comment text is never rewrapped and a trailing comment is never moved off its
line, so a line whose length comes from comment text, or from a single atom too
long to split, can exceed `width`. The suite asserts that nothing else does.

## Working with the parser

Two properties of `steel-parser` shaped the design.

`steel-parser`'s AST drops comments. `Parser` pulls its tokens with comments
enabled but then discards them (`TokenType::Comment => {}` in
`crates/steel-parser/src/parser.rs`), and `;;@doc` blocks and `#;` datum
comments do not reach `ExprKind` either. So steelwool does not format from the
AST. It builds its own concrete syntax tree from `lexer::TokenStream` with
`skip_comments` off, where comments are ordinary nodes. `TokenType::Comment`
carries no text of its own, but `Token::source` holds the exact slice, which is
what the tree stores. A line comment's token span includes its terminating
newline, so steelwool trims that back in order to keep seeing blank lines that
follow a comment.

`ExprKind`'s derived `PartialEq` is unusable as an equivalence oracle.
`List::syntax_object_id`, `LambdaFunction::syntax_object_id` and
`List::location` all take part in it, and the ids are freshly allocated per
parse, so two parses of the same text compare unequal. Hence the datum-tree
comparison above rather than `before == after` on the AST.

## Development

```sh
make check        # fmt-check, lint, test: the same gate as hooks/pre-push and CI
make fmt
make bless        # rewrite tests/data/*.expected from the current formatter
```

`.envrc` installs `hooks/pre-push` on first `direnv allow`.

## Licence

LGPL-3.0-or-later. See `COPYING.LESSER` and `COPYING`.
