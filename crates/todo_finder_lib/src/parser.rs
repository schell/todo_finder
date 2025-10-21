use nom::{
    bytes::complete as bytes, character::complete as character, combinator, IResult, Parser,
};
use snafu::ResultExt;
use tokio::io::AsyncReadExt;

use crate::{Error, IoSnafu, Message, NomSnafu, PrefixSnafu};

use super::{finder::FileSearcher, github::GitHubPatch};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

pub mod issue;
pub mod langs;
pub mod source;

use issue::GitHubTodoLocation;
use source::ParsedTodo;

/// Eat a whole line and optionally its ending but don't return that ending.
pub fn take_to_eol(i: &str) -> IResult<&str, &str> {
    let (i, ln) = bytes::take_till(|c| c == '\r' || c == '\n')(i)?;
    let (i, _) = combinator::opt(character::line_ending).parse(i)?;
    Ok((i, ln))
}

#[derive(Debug, Deserialize, Clone)]
pub enum IssueProvider {
    GitHub,
}

#[derive(Debug, Clone)]
pub enum ParsingSource {
    MarkdownFile,
    SourceCode,
    IssueAt(IssueProvider),
}

#[derive(Debug, Clone)]
pub struct IssueHead<K> {
    pub title: String,
    pub assignees: Vec<String>,
    pub external_id: K,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IssueBody<T> {
    pub descs_and_srcs: Vec<(Vec<String>, T)>,
}

impl IssueBody<FileTodoLocation> {
    pub fn to_github_string(
        &self,
        cwd: &str,
        owner: &str,
        repo: &str,
        checkout: &str,
    ) -> Result<String, Error> {
        let mut lines: Vec<String> = vec![];
        for (desc_lines, loc) in self.descs_and_srcs.iter() {
            let desc = desc_lines.clone().join("\n");
            let link = loc.to_github_link(cwd, owner, repo, checkout)?;
            lines.push([desc, link].join("\n"));
        }
        Ok(lines.join("\n"))
    }
}

#[derive(Debug, Clone)]
pub struct Issue<ExternalId, TodoLocation: PartialEq + Eq> {
    pub head: IssueHead<ExternalId>,
    pub body: IssueBody<TodoLocation>,
}

impl<ExId, Loc: PartialEq + Eq> Issue<ExId, Loc> {
    pub fn new(id: ExId, title: String) -> Self {
        Issue {
            head: IssueHead {
                title,
                assignees: vec![],
                external_id: id,
            },
            body: IssueBody {
                descs_and_srcs: vec![],
            },
        }
    }
}

/// A todo location in the local filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileTodoLocation {
    pub file: String,
    pub src_span: (usize, Option<usize>),
}

impl FileTodoLocation {
    /// ```rust
    /// use todo_finder_lib::parser::FileTodoLocation;
    ///
    /// let loc = FileTodoLocation {
    ///     file: "/total/path/src/file.rs".into(),
    ///     src_span: (666, Some(1337)),
    /// };
    ///
    /// let string = loc
    ///     .to_github_link("/total/path", "schell", "my_repo", "1234567890")
    ///     .unwrap();
    ///
    /// assert_eq!(
    ///     &string,
    ///     "https://github.com/schell/my_repo/blob/1234567890/src/file.rs#L666-L1337"
    /// );
    /// ```
    pub fn to_github_link(
        &self,
        cwd: &str,
        owner: &str,
        repo: &str,
        checkout: &str,
    ) -> Result<String, Error> {
        let path: &Path = Path::new(&self.file);
        let relative: &Path = path.strip_prefix(cwd).context(PrefixSnafu {
            path: path.to_path_buf(),
        })?;
        let file_and_range = [
            format!("{}", relative.display()),
            format!("#L{}", self.src_span.0),
            if let Some(end) = self.src_span.1 {
                format!("-L{}", end)
            } else {
                String::new()
            },
        ]
        .concat();

        let parts = [
            "https://github.com",
            owner,
            repo,
            "blob",
            checkout,
            &file_and_range,
        ];
        Ok(parts.join("/"))
    }
}

#[derive(Debug, Clone)]
pub struct IssueMap<ExternalId, TodoLocation: PartialEq + Eq> {
    pub parsed_from: ParsingSource,
    pub todos: HashMap<String, Issue<ExternalId, TodoLocation>>,
}

impl<K, V: Eq> IssueMap<K, V> {
    pub fn new(parsed_from: ParsingSource) -> IssueMap<K, V> {
        IssueMap {
            parsed_from,
            todos: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.todos.is_empty()
    }
}

impl IssueMap<u64, GitHubTodoLocation> {
    pub fn new_github_todos() -> Self {
        IssueMap {
            parsed_from: ParsingSource::IssueAt(IssueProvider::GitHub),
            todos: HashMap::new(),
        }
    }

    pub fn add_issue(&mut self, github_issue: &octocrab::models::issues::Issue) {
        if let Some(body) = github_issue.body.as_ref() {
            if let Ok((_, body)) = issue::issue_body(body) {
                let mut issue = Issue::new(github_issue.number, github_issue.title.clone());
                issue.body = body;
                self.todos.insert(github_issue.title.clone(), issue);
            }
        }
    }

    pub fn prepare_patch(&self, local: IssueMap<(), FileTodoLocation>) -> GitHubPatch {
        let mut create = IssueMap::new_source_todos();
        let mut edit: IssueMap<u64, FileTodoLocation> = IssueMap::new(ParsingSource::SourceCode);
        let mut dont_delete = vec![];

        for (title, local_issue) in local.todos.into_iter() {
            if let Some(remote_issue) = self.todos.get(&title) {
                // They both have it
                let id = remote_issue.head.external_id;
                dont_delete.push(id);
                let issue = Issue {
                    head: remote_issue.head.clone(),
                    body: local_issue.body,
                };
                edit.todos.insert(title, issue);
            } else {
                // Must be created
                create.todos.insert(title, local_issue);
            }
        }

        let delete = self
            .todos
            .values()
            .filter_map(|issue| {
                let id = issue.head.external_id;
                if dont_delete.contains(&id) {
                    None
                } else {
                    Some(id)
                }
            })
            .collect::<Vec<_>>();

        GitHubPatch {
            create,
            edit,
            delete,
        }
    }
}

impl<K> IssueMap<K, FileTodoLocation> {
    pub fn distinct_len(&self) -> usize {
        self.todos.len()
    }

    pub fn total_len(&self) -> usize {
        self.todos
            .values()
            .map(|issue| issue.body.descs_and_srcs.len())
            .sum()
    }
}

impl IssueMap<(), FileTodoLocation> {
    pub fn new_source_todos() -> Self {
        IssueMap {
            parsed_from: ParsingSource::SourceCode,
            todos: HashMap::new(),
        }
    }

    pub fn add_parsed_todo(&mut self, todo: &ParsedTodo, loc: FileTodoLocation) {
        let title = todo.title.to_string();
        let issue = self
            .todos
            .entry(title.clone())
            .or_insert(Issue::new((), title));

        if let Some(assignee) = todo.assignee.map(|s| s.to_string()) {
            if !issue.head.assignees.contains(&assignee) {
                issue.head.assignees.push(assignee);
            }
        }

        let desc_lines = todo
            .desc_lines
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        issue.body.descs_and_srcs.push((desc_lines, loc));
    }

    pub async fn from_files_in_directory(
        dir: &str,
        excludes: &[String],
    ) -> Result<IssueMap<(), FileTodoLocation>, Error> {
        Message::FindingTodosInSourceCode.send();

        let possible_todos = FileSearcher::find(dir, excludes).await?;
        let mut todos = IssueMap::new_source_todos();
        let language_map = langs::language_map();

        for possible_todo in possible_todos.into_iter() {
            let path = Path::new(&possible_todo.file);

            // Get our parser for this extension
            let ext: Option<_> = path.extension();
            if ext.is_none() {
                continue;
            }
            let ext = ext
                .expect("impossible!")
                .to_str()
                .expect("could not get extension as str")
                .to_owned();
            let languages = language_map.get(&ext);
            if languages.is_none() {
                Message::UnsupportedFile {
                    path: path.to_path_buf(),
                    todo: format!(
                        "line{}",
                        if possible_todo.lines_to_search.len() == 1 {
                            format!(" {}", possible_todo.lines_to_search.first().unwrap())
                        } else {
                            format!(
                                "s {}",
                                possible_todo
                                    .lines_to_search
                                    .iter()
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    ),
                }
                .send();
                continue;
            }
            let languages = languages.expect("impossible!");

            // Open the file and load the contents
            log::trace!("Reading {path:?}");
            let mut file = tokio::fs::File::open(path).await.context(IoSnafu)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).await.context(IoSnafu)?;

            let mut current_line = 1;
            let mut i = contents.as_str();
            for line in possible_todo.lines_to_search.into_iter() {
                // Seek to the correct line...
                while line > current_line {
                    let (j, _) = take_to_eol(i).map_err(|e| e.to_owned()).context(NomSnafu {
                        msg: "couldn't take line",
                    })?;
                    i = j;
                    current_line += 1;
                }
                log::trace!(
                    "  attempting to parse line {current_line}: '{}'",
                    i.lines().next().unwrap_or_default()
                );

                // Try parsing in each language until we get a match
                for language in languages.iter() {
                    if language.file_extensions.contains(&ext) {
                        log::trace!("Extension {ext} matches language {}", language.name);
                    }
                    let parser_config = language.as_todo_parser_config();
                    let parser = source::parse_todo(parser_config);
                    if let Ok((j, parsed_todo)) = parser(i) {
                        let num_lines = i.trim_end_matches(j).lines().fold(0, |n, _| n + 1);
                        let loc = FileTodoLocation {
                            file: possible_todo.file.to_string(),
                            src_span: (
                                line,
                                if num_lines > 1 {
                                    Some(line + num_lines - 1)
                                } else {
                                    None
                                },
                            ),
                        };
                        todos.add_parsed_todo(&parsed_todo, loc);
                        Message::FoundTodo.send();
                    }
                }
            }
        }

        Message::FoundTodos {
            distinct: todos.distinct_len(),
            total: todos.total_len(),
            markdown_text: todos.as_markdown(),
        }
        .send();
        Ok(todos)
    }

    pub fn as_markdown(&self) -> String {
        let num_distinct = self.todos.len();
        let num_locs = self
            .todos
            .values()
            .fold(0, |n, todo| n + todo.body.descs_and_srcs.len());

        let mut lines = vec![];

        lines.push("# TODOs".into());
        lines.push(format!(
            "Found {} distinct TODOs in {} file locations.\n",
            num_distinct, num_locs
        ));

        let mut todos = self.todos.clone().into_iter().collect::<Vec<_>>();
        todos.sort_by(|a, b| a.0.cmp(&b.0));

        for ((title, issue), n) in todos.into_iter().zip(1..) {
            lines.push(format!("{}. {}", n, title));
            for (descs, loc) in issue.body.descs_and_srcs.into_iter() {
                for line in descs.into_iter() {
                    lines.push(format!("  {}", line));
                }
                lines.push(format!(
                    "  file://{} ({})",
                    loc.file,
                    if let Some(end) = loc.src_span.1 {
                        format!("lines {} - {}", loc.src_span.0, end)
                    } else {
                        format!("line {}", loc.src_span.0)
                    },
                ));
                lines.push("".into());
            }
            if !issue.head.assignees.is_empty() {
                lines.push(format!(
                    "  assignees: {}\n",
                    issue.head.assignees.join(", ")
                ));
            }
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod test {
    use crate::parser::langs::rust_lang;

    #[test]
    fn rust_todo() {
        let input = r#"todo!("A special Rust-only todo");"#;
        let language = rust_lang();
        let parser_config = language.as_todo_parser_config();
        let parser = super::source::parse_todo(parser_config);

        let (_i, parsed) = parser(input).unwrap();
        println!("{parsed:#?}");
    }

    #[test]
    fn rust_todo_multi() {
        let input = r#"todo!(
            "A special Rust-only todo"
        );"#;
        let language = rust_lang();
        let parser_config = language.as_todo_parser_config();
        let parser = super::source::parse_todo(parser_config);

        let (_i, parsed) = parser(input).unwrap();
        println!("{parsed:#?}");
    }

    #[test]
    fn rust_todo_multi_multi() {
        let input = r#"todo!(
    "A special Rust-only todo on \
    more than one line, as a multi-line string \
    that is pretty long."
);"#;
        let language = rust_lang();
        let parser_config = language.as_todo_parser_config();
        let parser = super::source::parse_todo(parser_config);

        let (_i, parsed) = parser(input).unwrap();
        println!("{parsed:#?}");
    }
}
