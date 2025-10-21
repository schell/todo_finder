use snafu::prelude::*;
use std::{borrow::Cow, sync::LazyLock};

pub mod finder;
pub mod github;
pub mod parser;
pub mod utils;

static CHAN: LazyLock<(
    async_channel::Sender<Message>,
    async_channel::Receiver<Message>,
)> = LazyLock::new(async_channel::unbounded);

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("GitHub error: {source}"))]
    Octocrab { source: octocrab::Error },

    #[snafu(display("IO error: {source}"))]
    Io { source: std::io::Error },

    #[snafu(display("Command failed: {cmd} {status}"))]
    Command {
        cmd: Cow<'static, str>,
        status: std::process::ExitStatus,
        stdout: String,
        stderr: String,
    },

    #[snafu(display("Could not parse owner and repo from the git config"))]
    ParseOwnerRepo,

    #[snafu(display("Rg output was not UTF-8: {source}"))]
    RgUtf8 { source: std::str::Utf8Error },

    #[snafu(display("Rg parse error: {source}"))]
    ParseRg {
        source: nom::Err<nom::error::Error<String>>,
    },

    #[snafu(display("Parse error - {msg}: {source}"))]
    Nom {
        msg: &'static str,
        source: nom::Err<nom::error::Error<String>>,
    },

    #[snafu(display("Could not relativize path {path:?}: {source}"))]
    Prefix {
        path: std::path::PathBuf,
        source: std::path::StripPrefixError,
    },
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source }
    }
}

impl From<octocrab::Error> for Error {
    fn from(source: octocrab::Error) -> Self {
        Self::Octocrab { source }
    }
}

pub(crate) type Result<T, E = Error> = core::result::Result<T, E>;

/// An external progress message sent from the todo finder.
pub enum Message {
    Error(Error),

    GettingOrigin,
    GotOrigin {
        origin: String,
    },

    GettingOwnerRepo,
    GotOwnerRepo {
        owner: String,
        repo: String,
    },

    GettingCheckoutHash,
    GotCheckoutHash {
        hash: String,
    },

    FindingTodosInSourceCode,
    UnsupportedFile {
        path: std::path::PathBuf,
        todo: String,
    },
    FoundTodo,
    FoundTodos {
        distinct: usize,
        total: usize,
        markdown_text: String,
    },

    GettingIssues,
    GotIssues {
        count: usize,
    },

    PreparedPatch {
        create: usize,
        update: usize,
        delete: usize,
        dry_run: bool,
    },
    ApplyingPatch {
        create: usize,
        update: usize,
        delete: usize,
    },
    AppliedPatchCreate {
        done: usize,
        total: usize,
    },
    AppliedPatchUpdate {
        done: usize,
        total: usize,
    },
    AppliedPatchDelete {
        done: usize,
        total: usize,
    },
    AppliedPatch,

    Goodbye,
}

impl Message {
    /// Send a status message to the outside world.
    pub fn send(self) {
        // UNWRAP: safe because this channel is unbounded.
        CHAN.0.try_send(self).unwrap();
    }

    /// Get a clone of the status message receiver.
    pub fn receiver() -> async_channel::Receiver<Message> {
        CHAN.1.clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
