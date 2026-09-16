# Changelog

All notable changes to steelwool. Pre-1.0: a breaking change may land in any
`0.y` bump.

## 0.2.0

- New default-on passes: `comment-style`, `quote-sugar`,
  `boolean-spelling`, and `sort-provide`.
- The equivalence oracle understands each allowed rewrite explicitly and
  catches panics from the pinned Steel parser rather than crashing.
- `--diff` and `--list-different` report changes without writing. They compose
  with `--check`.
- Source errors report one-based `PATH:LINE:COLUMN` locations. File inputs are
  formatted in parallel, reported in argument order, and one failure no longer
  prevents the remaining inputs from being processed.
- Property tests generate Steel source and check equivalence, idempotence,
  width, and panic freedom in addition to the golden and real-corpus suites.
- Normative layout, pass, correctness, and CLI specifications live under
  `docs/spec/`.

## 0.1.0

First release.

- Format Steel source from a concrete syntax tree built on the `steel-parser`
  token stream, so comments survive.
- Layout: two space indentation, hanging indents under special forms, aligned
  hanging indents under calls, wrapping at `width` (default 80).
- Passes, each a boolean in `[passes]`, each defaulting to the pedantic choice:
  `provide-one-per-line`, `sort-require`, `attach-doc-comments`, `blank-lines`,
  `align-let-bindings`.
- `;; fmt: off` and `;; fmt: on` reproduce the enclosing region byte for byte.
- CLI: positional files formatted in place, stdin with no files or `-`,
  `--check`, `--config`, `--config-toml`, `--set KEY.PATH=VALUE`.
- Configuration discovery: `steelwool.toml` or `.steelwool.toml` upward to the
  git repository root, then `$XDG_CONFIG_HOME/steelwool/steelwool.toml`.
  Unrecognised keys are rejected.
- Parse equivalence is checked on every format and idempotence is checked
  across the whole test suite, including the ten Steel cogs from the Helix
  fork.

### Compatibility

- `steel-parser` pinned to `mattwparas/steel` rev
  `1b785a4e9d24e3553b242522b35d4498dae72816`, the revision the
  `steel-event-system` branch of `mattwparas/helix` uses.
