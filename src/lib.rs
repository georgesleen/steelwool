pub mod config;
pub mod cst;
pub mod layout;
pub mod passes;
pub mod verify;

pub use config::Config;

use std::fmt;

/// Everything that can stop steelwool from producing output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The lexer rejected the input.
    Lex { message: String, offset: u32 },
    /// A list was opened and never closed.
    UnclosedList { offset: u32 },
    /// A closing delimiter with no matching opening delimiter.
    UnexpectedClose { offset: u32 },
    /// A closing delimiter of a different shape than its opener.
    MismatchedDelimiter { offset: u32 },
    /// A quote or unquote prefix with no datum after it.
    DanglingPrefix { offset: u32 },
    /// Formatting produced a different program.
    ProgramChanged,
    /// Formatting produced output the Steel parser rejects.
    OutputRejected(String),
    /// Formatting is not stable after a second pass.
    NotIdempotent,
    /// The configuration is unusable.
    Config(config::ConfigError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Lex { message, offset } => {
                write!(f, "byte {offset}: {message}")
            }
            Error::UnclosedList { offset } => {
                write!(f, "byte {offset}: unclosed list")
            }
            Error::UnexpectedClose { offset } => {
                write!(f, "byte {offset}: unexpected closing delimiter")
            }
            Error::MismatchedDelimiter { offset } => {
                write!(f, "byte {offset}: mismatched closing delimiter")
            }
            Error::DanglingPrefix { offset } => {
                write!(f, "byte {offset}: quote prefix with nothing after it")
            }
            Error::ProgramChanged => {
                write!(f, "internal error: formatting changed the program")
            }
            Error::OutputRejected(message) => write!(
                f,
                "internal error: the Steel parser rejected the output: {message}"
            ),
            Error::NotIdempotent => {
                write!(f, "internal error: formatting is not idempotent")
            }
            Error::Config(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<config::ConfigError> for Error {
    fn from(error: config::ConfigError) -> Self {
        Error::Config(error)
    }
}

/// Formats `source`, refusing to return output that is not the same program.
pub fn format_source(source: &str, config: &Config) -> Result<String, Error> {
    let mut nodes = cst::build(source)?;
    passes::apply(&mut nodes, config);
    let formatted = layout::render(&nodes, config);
    verify::check_equivalence(source, &formatted)?;
    Ok(formatted)
}
