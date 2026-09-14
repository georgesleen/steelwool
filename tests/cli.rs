//! The command line contract: stdin, in place rewrites, exit codes, layering.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir()
            .join(format!("steelwool-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch directory is creatable");
        Scratch { path }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent is creatable");
        }
        std::fs::write(&path, contents).expect("scratch file is writable");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Runs the binary with the user configuration directory pointed at an empty
/// scratch path, so discovery never reaches the real one.
fn steelwool(arguments: &[&str], stdin: &str, directory: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_steelwool"))
        .args(arguments)
        .current_dir(directory)
        .env("XDG_CONFIG_HOME", directory.join("empty-config-home"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("steelwool starts");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("stdin is writable");
    child.wait_with_output().expect("steelwool finishes")
}

fn stdout_of(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("stdout is utf8")
}

fn stderr_of(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).expect("stderr is utf8")
}

#[test]
fn stdin_is_formatted_to_stdout() {
    let scratch = Scratch::new("stdin");
    let output = steelwool(&["-"], "(define(f x)(+ x 1))\n", &scratch.path);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "(define (f x) (+ x 1))\n");
}

#[test]
fn no_arguments_reads_stdin() {
    let scratch = Scratch::new("bare");
    let output = steelwool(&[], "(define  x   1)\n", &scratch.path);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(stdout_of(&output), "(define x 1)\n");
}

#[test]
fn check_exits_one_on_unformatted_input_and_zero_on_formatted() {
    let scratch = Scratch::new("check");
    let unformatted =
        steelwool(&["--check", "-"], "(define(f x) 1)\n", &scratch.path);
    assert_eq!(unformatted.status.code(), Some(1));
    assert!(stdout_of(&unformatted).is_empty());

    let formatted =
        steelwool(&["--check", "-"], "(define (f x) 1)\n", &scratch.path);
    assert_eq!(formatted.status.code(), Some(0));
    assert!(stdout_of(&formatted).is_empty());
}

#[test]
fn check_writes_nothing_to_the_file() {
    let scratch = Scratch::new("check-file");
    let source = "(define(f x) 1)\n";
    let path = scratch.write("input.scm", source);
    let output = steelwool(
        &["--check", path.to_str().expect("utf8 path")],
        "",
        &scratch.path,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr_of(&output).contains("would reformat"));
    assert_eq!(
        std::fs::read_to_string(&path).expect("file is readable"),
        source
    );
}

#[test]
fn files_are_formatted_in_place() {
    let scratch = Scratch::new("in-place");
    let first = scratch.write("first.scm", "(define(f x) 1)\n");
    let second = scratch.write("second.scm", "(define   y    2)\n");
    let output = steelwool(
        &[
            first.to_str().expect("utf8 path"),
            second.to_str().expect("utf8 path"),
        ],
        "",
        &scratch.path,
    );
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        std::fs::read_to_string(&first).expect("file is readable"),
        "(define (f x) 1)\n"
    );
    assert_eq!(
        std::fs::read_to_string(&second).expect("file is readable"),
        "(define y 2)\n"
    );
}

#[test]
fn set_overrides_a_dotted_key() {
    let scratch = Scratch::new("set");
    let source = "(provide alpha beta)\n";
    let kept = steelwool(&["-"], source, &scratch.path);
    assert_eq!(stdout_of(&kept), "(provide alpha\n         beta)\n");

    let overridden = steelwool(
        &["--set", "passes.provide-one-per-line=false", "-"],
        source,
        &scratch.path,
    );
    assert!(overridden.status.success(), "{}", stderr_of(&overridden));
    assert_eq!(stdout_of(&overridden), source);
}

#[test]
fn set_takes_precedence_over_config_toml() {
    let scratch = Scratch::new("precedence");
    let source = "(define (join a b c) (list a b c))\n";
    let output = steelwool(
        &["--config-toml", "width = 200", "--set", "width=20", "-"],
        source,
        &scratch.path,
    );
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        stdout_of(&output),
        "(define (join a b c)\n  (list a b c))\n"
    );
}

#[test]
fn a_config_file_is_discovered_from_the_input_directory() {
    let scratch = Scratch::new("discovery");
    scratch.write(".git/HEAD", "ref: refs/heads/main\n");
    scratch.write("steelwool.toml", "width = 30\n");
    let path = scratch.write(
        "nested/deeper/input.scm",
        "(define (join a b c) (list a b c))\n",
    );
    let output =
        steelwool(&[path.to_str().expect("utf8 path")], "", &scratch.path);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        std::fs::read_to_string(&path).expect("file is readable"),
        "(define (join a b c)\n  (list a b c))\n"
    );
}

#[test]
fn discovery_stops_at_the_repository_root() {
    let scratch = Scratch::new("root-stop");
    scratch.write("steelwool.toml", "width = 30\n");
    scratch.write("repo/.git/HEAD", "ref: refs/heads/main\n");
    let path =
        scratch.write("repo/input.scm", "(define (join a b c) (list a b c))\n");
    let output =
        steelwool(&[path.to_str().expect("utf8 path")], "", &scratch.path);
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        std::fs::read_to_string(&path).expect("file is readable"),
        "(define (join a b c) (list a b c))\n"
    );
}

#[test]
fn an_explicit_config_replaces_discovery() {
    let scratch = Scratch::new("explicit");
    scratch.write(".git/HEAD", "ref: refs/heads/main\n");
    scratch.write("steelwool.toml", "width = 20\n");
    let explicit = scratch.write("other.toml", "width = 200\n");
    let path =
        scratch.write("input.scm", "(define (join a b c) (list a b c))\n");
    let output = steelwool(
        &[
            "--config",
            explicit.to_str().expect("utf8 path"),
            path.to_str().expect("utf8 path"),
        ],
        "",
        &scratch.path,
    );
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert_eq!(
        std::fs::read_to_string(&path).expect("file is readable"),
        "(define (join a b c) (list a b c))\n"
    );
}

#[test]
fn an_unknown_configuration_key_is_rejected_loudly() {
    let scratch = Scratch::new("unknown-key");
    let output = steelwool(
        &["--config-toml", "indent = 4", "-"],
        "(define x 1)\n",
        &scratch.path,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).contains("unknown configuration key `indent`"),
        "{}",
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
}

#[test]
fn an_unknown_pass_name_is_rejected_loudly() {
    let scratch = Scratch::new("unknown-pass");
    let output = steelwool(
        &["--set", "passes.sort-provide=true", "-"],
        "(define x 1)\n",
        &scratch.path,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output)
            .contains("unknown configuration key `passes.sort-provide`"),
        "{}",
        stderr_of(&output)
    );
}

#[test]
fn a_malformed_set_is_rejected() {
    let scratch = Scratch::new("bad-set");
    let output =
        steelwool(&["--set", "width", "-"], "(define x 1)\n", &scratch.path);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).contains("--set expects"),
        "{}",
        stderr_of(&output)
    );
}

#[test]
fn unbalanced_input_is_reported_and_nothing_is_written() {
    let scratch = Scratch::new("unbalanced");
    let source = "(define (f x)\n";
    let path = scratch.write("broken.scm", source);
    let output =
        steelwool(&[path.to_str().expect("utf8 path")], "", &scratch.path);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).contains("unclosed list"),
        "{}",
        stderr_of(&output)
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("file is readable"),
        source
    );
}
