//! Running ripgrep to find TODOs.
use snafu::ResultExt;

use crate::{utils::get_rg_output, Error, ParseRgSnafu, RgUtf8Snafu};

use super::parse;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PossibleTodosInFile {
    pub file: String,
    pub lines_to_search: Vec<usize>,
}

impl PossibleTodosInFile {
    pub fn new(file: &str, lines_to_search: Vec<usize>) -> Self {
        PossibleTodosInFile {
            file: file.into(),
            lines_to_search,
        }
    }
}

/// Parse the output of `rg` into a map of file to possible todo locations.
pub(crate) fn parse_rg_output(output: &[u8]) -> Result<Vec<PossibleTodosInFile>, Error> {
    let rg_output = std::str::from_utf8(output).context(RgUtf8Snafu)?;

    let (_, files) = parse::parse_rg(rg_output)
        .map_err(|e| e.to_owned())
        .context(ParseRgSnafu)?;

    let mut todos: Vec<_> = files
        .into_iter()
        .map(|(file, lines)| PossibleTodosInFile::new(file, lines))
        .collect();
    todos.sort();

    Ok(todos)
}

/// Run `rg` with the path and some commonly used TODO patterns, returning the
/// result bytes if successful.
pub async fn get_rg_output_with_common_patterns(
    path: &str,
    excludes: &[String],
) -> Result<Vec<u8>, Error> {
    let mut todos = vec![];
    for pattern in crate::parser::source::TAG_PATTERNS {
        todos.extend(get_rg_output(path, pattern, excludes).await?);
    }

    Ok(todos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn can_search_todos_in_files() {
        let may_output = Command::new("rg")
            .arg("--heading")
            .arg("--line-number")
            .arg("TODO")
            .arg("test_data")
            .output();

        assert!(may_output.is_ok());
        let output = may_output.unwrap();
        assert!(output.status.success());

        let may_files = parse_rg_output(&output.stdout);
        assert!(may_files.is_ok());
        let files = may_files.unwrap();
        assert_eq!(
            files,
            vec![
                PossibleTodosInFile {
                    file: "test_data/one.rs".into(),
                    lines_to_search: vec![1, 13, 30],
                },
                PossibleTodosInFile {
                    file: "test_data/two.rs".into(),
                    lines_to_search: vec![1, 13, 15, 32],
                },
            ]
        )
    }

    #[test]
    fn can_search_rust_todos_in_files() {
        let may_output = Command::new("rg")
            .arg("--heading")
            .arg("--line-number")
            .arg("todo!")
            .arg("test_data")
            .output();

        assert!(may_output.is_ok());
        let output = may_output.unwrap();
        assert!(output.status.success());

        let may_files = parse_rg_output(&output.stdout);
        assert!(may_files.is_ok());
        let files = may_files.unwrap();
        assert_eq!(
            vec![PossibleTodosInFile {
                file: "test_data/one.rs".into(),
                lines_to_search: vec![33, 34, 38],
            }],
            files,
        )
    }
}
