//! Opinionated defaults, one layout knob, and a named boolean per pass.

use std::fmt;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

pub const FILE_NAMES: [&str; 2] = ["steelwool.toml", ".steelwool.toml"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The only layout knob. Indentation is fixed at two spaces.
    pub width: usize,
    pub passes: Passes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passes {
    /// One exported name per line in a `provide` form.
    pub provide_one_per_line: bool,
    /// Sort clauses within a `require` form and forms within a require block.
    pub sort_require: bool,
    /// Keep a `;;@doc` comment block glued to the form it documents.
    pub attach_doc_comments: bool,
    /// Exactly one blank line between top-level forms.
    pub blank_lines: bool,
    /// Align `let` bindings under the first binding.
    pub align_let_bindings: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            width: 80,
            passes: Passes::default(),
        }
    }
}

impl Default for Passes {
    fn default() -> Self {
        Passes {
            provide_one_per_line: true,
            sort_require: true,
            attach_doc_comments: true,
            blank_lines: true,
            align_let_bindings: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Syntax {
        origin: String,
        message: String,
    },
    UnknownKey {
        origin: String,
        key: String,
    },
    WrongType {
        origin: String,
        key: String,
        expected: &'static str,
    },
    BadWidth {
        origin: String,
        width: i64,
    },
    NotATable {
        origin: String,
        key: String,
    },
    Unreadable {
        path: PathBuf,
        message: String,
    },
    BadSet {
        argument: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Syntax { origin, message } => {
                write!(f, "{origin}: {message}")
            }
            ConfigError::UnknownKey { origin, key } => {
                write!(f, "{origin}: unknown configuration key `{key}`")
            }
            ConfigError::WrongType {
                origin,
                key,
                expected,
            } => write!(f, "{origin}: `{key}` must be {expected}"),
            ConfigError::BadWidth { origin, width } => {
                write!(f, "{origin}: `width` must be at least 1, got {width}")
            }
            ConfigError::NotATable { origin, key } => {
                write!(f, "{origin}: `{key}` must be a table")
            }
            ConfigError::Unreadable { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            ConfigError::BadSet { argument } => write!(
                f,
                "--set expects KEY.PATH=VALUE with a TOML value, got `{argument}`"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// A stack of TOML layers, lowest precedence first.
#[derive(Debug, Default)]
pub struct Layers {
    merged: Table,
    origin: String,
}

impl Layers {
    pub fn new() -> Self {
        Layers {
            merged: Table::new(),
            origin: "configuration".to_string(),
        }
    }

    pub fn push_file(&mut self, path: &Path) -> Result<(), ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|error| {
            ConfigError::Unreadable {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        self.push_toml(&text, &path.display().to_string())
    }

    pub fn push_toml(
        &mut self,
        text: &str,
        origin: &str,
    ) -> Result<(), ConfigError> {
        let table: Table = text.parse().map_err(|error: toml::de::Error| {
            ConfigError::Syntax {
                origin: origin.to_string(),
                message: error.message().to_string(),
            }
        })?;
        merge(&mut self.merged, table);
        self.origin = origin.to_string();
        Ok(())
    }

    /// Applies a `KEY.PATH=VALUE` override, where `VALUE` is TOML.
    pub fn push_set(&mut self, argument: &str) -> Result<(), ConfigError> {
        let (key, value) =
            argument
                .split_once('=')
                .ok_or_else(|| ConfigError::BadSet {
                    argument: argument.to_string(),
                })?;
        let key = key.trim();
        if key.is_empty() {
            return Err(ConfigError::BadSet {
                argument: argument.to_string(),
            });
        }
        let document = format!("{key} = {value}");
        let table: Table =
            document.parse().map_err(|_| ConfigError::BadSet {
                argument: argument.to_string(),
            })?;
        merge(&mut self.merged, table);
        self.origin = format!("--set {argument}");
        Ok(())
    }

    pub fn resolve(&self) -> Result<Config, ConfigError> {
        from_table(&self.merged, &self.origin)
    }
}

fn merge(into: &mut Table, from: Table) {
    for (key, value) in from {
        match (into.get_mut(&key), value) {
            (Some(Value::Table(existing)), Value::Table(incoming)) => {
                merge(existing, incoming)
            }
            (_, value) => {
                into.insert(key, value);
            }
        }
    }
}

fn boolean(
    table: &Table,
    key: &str,
    origin: &str,
    field: &mut bool,
) -> Result<(), ConfigError> {
    match table.get(key) {
        None => Ok(()),
        Some(Value::Boolean(value)) => {
            *field = *value;
            Ok(())
        }
        Some(_) => Err(ConfigError::WrongType {
            origin: origin.to_string(),
            key: key.to_string(),
            expected: "a boolean",
        }),
    }
}

/// Parses `table` into a `Config`, rejecting any key it does not recognise.
pub fn from_table(table: &Table, origin: &str) -> Result<Config, ConfigError> {
    let mut config = Config::default();

    match table.get("width") {
        None => {}
        Some(Value::Integer(width)) => {
            if *width < 1 {
                return Err(ConfigError::BadWidth {
                    origin: origin.to_string(),
                    width: *width,
                });
            }
            config.width = *width as usize;
        }
        Some(_) => {
            return Err(ConfigError::WrongType {
                origin: origin.to_string(),
                key: "width".to_string(),
                expected: "an integer",
            });
        }
    }

    let passes = match table.get("passes") {
        None => Table::new(),
        Some(Value::Table(passes)) => passes.clone(),
        Some(_) => {
            return Err(ConfigError::NotATable {
                origin: origin.to_string(),
                key: "passes".to_string(),
            });
        }
    };

    boolean(
        &passes,
        "provide-one-per-line",
        origin,
        &mut config.passes.provide_one_per_line,
    )?;
    boolean(
        &passes,
        "sort-require",
        origin,
        &mut config.passes.sort_require,
    )?;
    boolean(
        &passes,
        "attach-doc-comments",
        origin,
        &mut config.passes.attach_doc_comments,
    )?;
    boolean(
        &passes,
        "blank-lines",
        origin,
        &mut config.passes.blank_lines,
    )?;
    boolean(
        &passes,
        "align-let-bindings",
        origin,
        &mut config.passes.align_let_bindings,
    )?;

    const KNOWN_PASSES: [&str; 5] = [
        "provide-one-per-line",
        "sort-require",
        "attach-doc-comments",
        "blank-lines",
        "align-let-bindings",
    ];
    for key in passes.keys() {
        if !KNOWN_PASSES.contains(&key.as_str()) {
            return Err(ConfigError::UnknownKey {
                origin: origin.to_string(),
                key: format!("passes.{key}"),
            });
        }
    }
    for key in table.keys() {
        if key != "width" && key != "passes" {
            return Err(ConfigError::UnknownKey {
                origin: origin.to_string(),
                key: key.clone(),
            });
        }
    }

    Ok(config)
}

/// Searches `start` and its ancestors up to and including the git repository
/// root, then the user configuration directory.
pub fn discover(start: &Path) -> Option<PathBuf> {
    let mut directory = Some(start);
    while let Some(current) = directory {
        for name in FILE_NAMES {
            let candidate = current.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        if current.join(".git").exists() {
            break;
        }
        directory = current.parent();
    }
    user_config()
}

fn user_config() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    let candidate = base.join("steelwool").join("steelwool.toml");
    candidate.is_file().then_some(candidate)
}
