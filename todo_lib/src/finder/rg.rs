//! Running ripgrep to find TODOs.
//!
use std::process::Command;

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


/// Run `rg` with the path and pattern given, returning the result bytes if
/// successful.
pub(crate) fn get_rg_output(
    path: &str,
    pattern: &str,
    excludes: &Vec<String>,
) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("rg".to_string());
    let _ = cmd
        .arg("--heading".to_string())
        .arg("--line-number".to_string());
    for exclude in excludes.iter() {
        cmd.arg("-g").arg(format!("!{}", exclude));
    }
    let _ = cmd.arg(pattern).arg(path);

    println!("running rg:\n{:#?}", cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("error using rg: {:#?}", e))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        // For some reason rg returns an error when there are no results...
        Ok(vec![])
    }
}


/// Parse the output of `rg` into a map of file to possible todo locations.
pub(crate) fn parse_rg_output(output: &Vec<u8>) -> Result<Vec<PossibleTodosInFile>, String> {
    let rg_output = std::str::from_utf8(output)
        .map_err(|e| format!("could not convert rg output to utf8: {:#?}", e))?;

    let (_, files) =
        parse::parse_rg(rg_output).map_err(|e| format!("rg nom parse error: {:#?}", e))?;

    let mut todos: Vec<_> = files
        .into_iter()
        .map(|(file, lines)| PossibleTodosInFile::new(file, lines))
        .collect();
    todos.sort();

    Ok(todos)
}


/// Run `rg` with the path and some commonly used TODO patterns, returning the
/// result bytes if successful.
pub(crate) fn get_rg_output_with_common_patterns(
    path: &str,
    excludes: &Vec<String>,
) -> Result<Vec<u8>, String> {
    let patterns = ["TODO", "@todo", "FIXME"];

    let mut todos = vec![];
    for pattern in patterns.iter() {
        todos.extend(get_rg_output(path, pattern, excludes)?);
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
}
