# Command line

The surface, the exit codes, and the shape of a diagnostic. Locked by
`tests/cli.rs`, which runs the built binary.

## Invocation

| Rule | Statement | Locked by |
| --- | --- | --- |
| C1 | `steelwool FILES...` formats each file in place. | `files_are_formatted_in_place` |
| C2 | With no files, or with `-`, steelwool reads stdin and writes stdout. | `stdin_is_formatted_to_stdout`, `no_arguments_reads_stdin` |
| C3 | An in place write happens only when the formatted text differs from the file, so an already formatted file is not touched. | `an_unchanged_file_is_not_rewritten` |
| C4 | `--` ends option parsing. An unknown option is an error. | `an_unknown_option_is_rejected` |
| C5 | `-h`, `--help` print usage to stdout and exit 0. `-V`, `--version` print the version and exit 0. | `help_and_version_exit_zero` |

## Modes that write nothing

| Rule | Statement | Locked by |
| --- | --- | --- |
| C6 | `--check` writes nothing and prints `would reformat PATH` to stderr for each input that would change. | `check_writes_nothing_to_the_file`, `check_exits_one_on_unformatted_input_and_zero_on_formatted` |
| C7 | `--diff` writes nothing and prints a unified diff of each changed input to stdout. | `diff_prints_a_unified_diff` |
| C8 | `--list-different` writes nothing and prints the path of each changed input to stdout, one per line, nothing else. | `list_different_prints_only_paths` |
| C9 | A diff hunk header is `@@ -OLD_START,OLD_COUNT +NEW_START,NEW_COUNT @@`, with three lines of context, under `--- PATH` and `+++ PATH` headers. The path for stdin is `<stdin>`. | `diff_prints_a_unified_diff` |
| C10 | The three modes compose: each changed input contributes to whichever of the three outputs was asked for. | `diff_and_list_different_compose` |

## Configuration

| Rule | Statement | Locked by |
| --- | --- | --- |
| C11 | Layers apply lowest to highest: built in defaults, the config file, `--config-toml`, then each `--set KEY.PATH=VALUE` in the order given. | `set_takes_precedence_over_config_toml`, `set_overrides_a_dotted_key` |
| C12 | Discovery, per input, looks for `steelwool.toml` then `.steelwool.toml` in the file's directory and each ancestor, stopping after the directory containing `.git`, then reads `$XDG_CONFIG_HOME/steelwool/steelwool.toml`. | `a_config_file_is_discovered_from_the_input_directory`, `discovery_stops_at_the_repository_root` |
| C13 | `--config PATH` replaces discovery entirely. | `an_explicit_config_replaces_discovery` |
| C14 | An unrecognised configuration key, an unrecognised pass name, a value of the wrong type, and a malformed `--set` are all hard errors. | `an_unknown_configuration_key_is_rejected_loudly`, `an_unknown_pass_name_is_rejected_loudly`, `a_malformed_set_is_rejected` |

## Diagnostics

| Rule | Statement | Locked by |
| --- | --- | --- |
| C15 | An error that has a source position is reported as `PATH:LINE:COLUMN: message`. Line and column are 1 based and the column is counted in characters. | `a_lex_error_is_reported_with_line_and_column` |
| C16 | An error with no source position is reported as `PATH: message`. The path for stdin is `<stdin>`. | `unbalanced_input_is_reported_and_nothing_is_written` |
| C17 | Every diagnostic goes to stderr prefixed `steelwool: `, except the outputs of C7 and C8, which are the tool's data and go to stdout unprefixed. | `diff_prints_a_unified_diff` |
| C18 | One unreadable or unformattable file does not stop the others: every input is attempted and every failure is reported once. | `one_bad_file_does_not_stop_the_others` |

## Exit codes

| Rule | Statement | Locked by |
| --- | --- | --- |
| C19 | 0 when nothing would change, 1 when some input would change under `--check`, `--diff` or `--list-different`, 2 on any error. | `check_exits_one_on_unformatted_input_and_zero_on_formatted` |
| C20 | 2 dominates 1: an error anywhere means exit 2 even when another input would also have changed. | `one_bad_file_does_not_stop_the_others` |

## Concurrency

| Rule | Statement | Locked by |
| --- | --- | --- |
| C21 | Files are formatted on up to `available_parallelism` threads. | `many_files_are_all_formatted` |
| C22 | Output is deterministic: diffs, paths and diagnostics appear in the order the files were given, whatever order they were formatted in. | `output_order_follows_argument_order` |
