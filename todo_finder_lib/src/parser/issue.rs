//! # Parsing todos from an issue.
use nom::{bytes::complete as bytes, character::complete as character, combinator, multi, IResult};
use std::collections::HashMap;

use super::{take_to_eol, IssueBody};

/// Version 0 of the source location parser.
/// Parse the file and line location from an issue decription.
pub fn src_location0(i: &str) -> IResult<&str, (&str, usize)> {
    let (i, _) = bytes::tag("Located in ")(i)?;
    let dub_quote = character::char('"');
    let (i, _) = dub_quote(i)?;
    let (i, file) = bytes::take_till(|c| c == '"')(i)?;
    let (i, _) = dub_quote(i)?;
    let (i, _) = bytes::tag(" on line ")(i)?;
    let (i, ln_str) = character::digit1(i)?;
    let n = ln_str
        .parse::<usize>()
        .expect("could not convert line number: src_location0");
    Ok((i, (file, n)))
}

/// Parse a user's name and repo name from a github style path.
pub fn repo_from_github_link(i: &str) -> IResult<&str, (&str, &str)> {
    let (i, user) = bytes::take_till(|c| c == '/')(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, repo) = bytes::take_till(|c| c == '/')(i)?;
    Ok((i, (user, repo)))
}

/// Parse a SpanLength from a GitHub link. Pass a number of lines to widen the
/// window of the code region.
///
/// ```rust
/// use todo_finder_lib::parser::issue::*;
///
/// let bytes = "#L7-L9";
/// assert_eq!(span_from_github_link(bytes), Ok(("", (7, Some(9)))));
/// ```
pub fn span_from_github_link(i: &str) -> IResult<&str, (usize, Option<usize>)> {
    let (i, _) = bytes::tag("#L")(i)?;
    let (i, ln_str) = character::digit1(i)?;
    let start = ln_str
        .parse::<usize>()
        .expect("could not convert line number: span_from_github_link");
    fn convert_line(ii: &str) -> IResult<&str, usize> {
        let (ii, _) = bytes::tag("-L")(ii)?;
        let (ii, ln_str) = character::digit1(ii)?;
        let end = ln_str
            .parse::<usize>()
            .expect("could not convert line number: span_from_github_link::fn");
        Ok((ii, end))
    }
    let (i, may_end) = combinator::opt(convert_line)(i)?;
    Ok((i, (start, may_end)))
}

/// Uniquely identifies a todo location.
#[derive(Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct GitHubTodoLocation {
    pub repo: (String, String),
    pub checkout: String,
    pub file: String,
    pub src_span: (usize, Option<usize>),
}

/// Parses the location of a todo from a github link.
///
/// ```rust
/// use todo_finder_lib::parser::issue::*;
///
/// let bytes = "https://github.com/schell/repo/blob/yar/File.hs#L666 ";
///
/// assert_eq!(
///     todo_location_from_github_link(bytes),
///     Ok((
///         " ",
///         GitHubTodoLocation {
///             repo: ("schell".into(), "repo".into()),
///             checkout: "yar".into(),
///             file: "File.hs".into(),
///             src_span: (666, None)
///         }
///     ))
/// );
/// ```
pub fn todo_location_from_github_link(i: &str) -> IResult<&str, GitHubTodoLocation> {
    let (i, _) = bytes::tag("https://github.com/")(i)?;
    let (i, repo) = repo_from_github_link(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, _) = bytes::tag("blob")(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, checkout) = bytes::take_till(|c| c == '/')(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, file) = bytes::take_till(|c| c == '#')(i)?;
    let (i, src_span) = span_from_github_link(i)?;
    Ok((
        i,
        GitHubTodoLocation {
            repo: (repo.0.into(), repo.1.into()),
            checkout: checkout.into(),
            file: file.into(),
            src_span,
        },
    ))
}

/// Parses the location of a todo from an issue's markdown link to the source
/// file provided in the issue body itself.
///
/// ```rust
/// use todo_finder_lib::parser::issue::*;
///
/// let bytes = "[stuff](https://github.com/schell/repo/blob/yar/File.hs#L666 \"aoeu\")\n";
///
/// assert_eq!(
///     todo_location_from_github_markdown_link(bytes),
///     Ok((
///         "\n",
///         GitHubTodoLocation {
///             repo: ("schell".into(), "repo".into()),
///             checkout: "yar".into(),
///             file: "File.hs".into(),
///             src_span: (666, None)
///         }
///     ))
/// );
/// ```
pub fn todo_location_from_github_markdown_link(i: &str) -> IResult<&str, GitHubTodoLocation> {
    let (i, may_tloc) = combinator::opt(todo_location_from_github_link)(i)?;
    if may_tloc.is_none() {
        let (i, _) = character::char('[')(i)?;
        let (i, _) = bytes::take_till(|c| c == ']')(i)?;
        let (i, _) = character::char(']')(i)?;
        let (i, _) = character::char('(')(i)?;
        let (i, tloc) = todo_location_from_github_link(i)?;
        let (i, _) = bytes::take_till(|c| c == ')')(i)?;
        let (i, _) = character::char(')')(i)?;
        Ok((i, tloc))
    } else {
        Ok((i, may_tloc.unwrap()))
    }
}

/// Holds a branch and whether a todo exists on said branch, or if it has been
/// removed from said branch.
#[derive(Clone, Debug, PartialEq)]
pub struct TodoStory<'a> {
    pub branch: &'a str,
    pub is_closed: bool,
}

impl<'a> TodoStory<'a> {
    pub fn is_open(&self) -> bool {
        !self.is_closed
    }
}

/// Parse a todo story from an issue comment.
///
/// ```rust
/// use nom::multi;
/// use todo_finder_lib::parser::issue::*;
///
/// let bytes = "\
/// * Opened on master
/// * Found on develop
/// * Closed on develop
/// * Closed on master
/// ";
///
/// assert_eq!(
///     multi::many1(todo_story)(bytes),
///     Ok((
///         "",
///         vec![
///             TodoStory {
///                 branch: "master",
///                 is_closed: false
///             },
///             TodoStory {
///                 branch: "develop",
///                 is_closed: false
///             },
///             TodoStory {
///                 branch: "develop",
///                 is_closed: true
///             },
///             TodoStory {
///                 branch: "master",
///                 is_closed: true
///             },
///         ]
///     ))
/// );
/// ```
pub fn todo_story(i: &str) -> IResult<&str, TodoStory> {
    let (i, _) = character::char('*')(i)?;
    let alts = [
        (" Opened on ", false),
        (" Found on ", false),
        (" Closed on ", true),
    ];

    let mut is_closed = true;
    let mut ii = i;
    'story: for (alt, alt_is_closed) in &alts {
        let (j, may_story) = combinator::opt(bytes::tag(*alt))(ii)?;
        ii = j;
        if may_story.is_some() {
            is_closed = *alt_is_closed;
            break 'story;
        }
    }

    let (i, branch) = take_to_eol(ii)?;
    Ok((i, TodoStory { branch, is_closed }))
}

/// Collapse and filter the input stories into a vector of branches that the todo
/// still exists on.
pub fn branches_todo_is_found_on(stories: Vec<TodoStory>) -> Vec<&str> {
    let mut map: HashMap<&str, bool> = HashMap::new();
    for story in stories.into_iter() {
        map.insert(story.branch, story.is_closed);
    }

    let mut branches = map
        .into_iter()
        .filter_map(
            |(branch, is_closed)| {
                if is_closed {
                    None
                } else {
                    Some(branch)
                }
            },
        )
        .collect::<Vec<_>>();
    branches.sort();
    branches
}

/// Parse a vector of TodoStory.
pub fn todo_stories(i: &str) -> IResult<&str, Vec<TodoStory>> {
    multi::many1(todo_story)(i)
}

/// Parse a todo from an issue.
/// Returns the location of the todo and the lines of the todo's description.
pub fn issue_todo(i: &str) -> IResult<&str, (Vec<&str>, GitHubTodoLocation)> {
    multi::many_till(take_to_eol, todo_location_from_github_markdown_link)(i)
}

/// Parse the entire body of an issue.
/// TODO: Remove stories entirely.
/// We really only need to operate on one branch.
pub fn issue_body(i: &str) -> IResult<&str, IssueBody<GitHubTodoLocation>> {
    let mut ii = i;
    let mut descs_todos = vec![];
    let mut may_stories = None;
    'todos: loop {
        let (j, desc_todo) = issue_todo(ii)?;
        descs_todos.push(desc_todo);
        let (j, _) = multi::many0(character::newline)(j)?;
        let (j, my_may_stories) = combinator::opt(todo_stories)(j)?;
        ii = j;
        if my_may_stories.is_some() {
            may_stories = my_may_stories;
            break 'todos;
        }
        if j.is_empty() {
            break 'todos;
        }
    }
    let mut descs_todos = descs_todos
        .into_iter()
        .map(|(descs, todos)| {
            (
                descs.into_iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                todos,
            )
        })
        .collect::<Vec<_>>();
    descs_todos.sort_by(|(_, a_loc), (_, b_loc)| a_loc.cmp(&b_loc));

    let branches = if let Some(stories) = may_stories {
        let mut branches: Vec<String> = branches_todo_is_found_on(stories)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        branches.sort();
        branches
    } else {
        vec![]
    };

    Ok((
        ii,
        IssueBody {
            descs_and_srcs: descs_todos,
            branches,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_todo_location_from_github_link() {
        let bytes: &str = "\
https://github.com/schell/src-of-truth/blob/\
b18659e607c3673b883b4caa07a1e850e0a6121c/src/SrcOfTruth.hs#L258";
        assert_eq!(
            todo_location_from_github_link(bytes),
            Ok((
                "",
                GitHubTodoLocation {
                    repo: ("schell".into(), "src-of-truth".into()),
                    checkout: "b18659e607c3673b883b4caa07a1e850e0a6121c".into(),
                    file: "src/SrcOfTruth.hs".into(),
                    src_span: (258, None)
                }
            ))
        );

        let bytes = "\
https://github.com/schell/src-of-truth/blob/\
a1eb484c90f9e0b85ab5066b8950750a5bd4ab95/app/Main.hs#L3-L7";

        assert_eq!(
            todo_location_from_github_link(bytes),
            Ok((
                "",
                GitHubTodoLocation {
                    repo: ("schell".into(), "src-of-truth".into()),
                    checkout: "a1eb484c90f9e0b85ab5066b8950750a5bd4ab95".into(),
                    file: "app/Main.hs".into(),
                    src_span: (3, Some(7))
                }
            ))
        )
    }

    #[test]
    fn can_parse_todo_location_with_range_from_github_link() {
        let bytes = "\
https://github.com/schell/src-of-truth/blob/\
6e2f663102a282027f1fb0cdf0f0c4e203a118f1/src/SrcOfTruth/Issues.hs#L254-L256\n\n";
        assert_eq!(
            todo_location_from_github_link(bytes),
            Ok((
                "\n\n",
                GitHubTodoLocation {
                    repo: ("schell".into(), "src-of-truth".into()),
                    checkout: "6e2f663102a282027f1fb0cdf0f0c4e203a118f1".into(),
                    file: "src/SrcOfTruth/Issues.hs".into(),
                    src_span: (254, Some(256))
                }
            ))
        );
    }

    #[test]
    fn can_parse_todo_stories() {
        let bytes = "\
* Opened on mitchell-diagram
* Found on master
* Found on module-headers
* Found on mitchell-resourcet
* Closed on module-headers
* Found on move-stylish
* Closed on mitchell-resourcet
* Found on pretty-gh-links
* Found on mitchell-gh
* Found on mitchell-pemfile-arg
* Closed on mitchell-pemfile-arg
* Found on feature-wip
* Found on mitchell-190
* Found on mitchell-191
* Found on mitchell-236
* Closed on mitchell-190
* Closed on mitchell-191
* Closed on mitchell-236
* Found on mitchell-241
* Found on restart-services
* Closed on feature-wip
* Closed on mitchell-241
* Closed on mitchell-256
* Closed on restart-services
* Closed on feature-pretty-gh-links
* Closed on feature-gh-pretty-links-2
* Closed on specific-runner-hack
* Closed on mitchell-gh
* Closed on feature-purescript-flavor
* Closed on metrics-class
* Closed on error-260
* Closed on feature-job-metrics
* Closed on master
* Closed on storyline-tests
* Closed on mitchell-gh-auth
* Closed on manager-retries
* Closed on revert-263-feature-pretty-gh-links
* Closed on pretty-gh-links
* Closed on mitchell-diagram
* Closed on comment-branches
* Closed on feature-mitchell-fetch-issues
";
        let may_stories = todo_stories(bytes);
        assert!(may_stories.is_ok());

        let (_, stories) = may_stories.unwrap();
        let branches = branches_todo_is_found_on(stories);
        assert_eq!(branches, vec!["move-stylish"]);
    }

    // TODO: round trip tests for parsing issues and writing them.
    #[test]
    fn can_parse_issue_todo() {
        let bytes = "\
This is the description.
[stuff](https://github.com/schell/repo/blob/abighash/src/File.hs#L666 \
                     \"title\")
";
        let may_desc_and_loc = issue_todo(bytes);
        assert!(may_desc_and_loc.is_ok());

        let (left, (desc, loc)) = may_desc_and_loc.unwrap();
        assert_eq!("\n", left, "leftover");
        assert_eq!(vec!["This is the description."], desc, "description");
        assert_eq!(
            GitHubTodoLocation {
                repo: ("schell".into(), "repo".into()),
                checkout: "abighash".into(),
                file: "src/File.hs".into(),
                src_span: (666, None)
            },
            loc,
            "location"
        );
    }

    #[test]
    pub fn will_not_parse_newline_as_space() {
        let bytes = "   \n   ";
        let res: IResult<&str, &str> = character::space0(bytes);
        assert_eq!(res, Ok(("\n   ", "   ")));
    }

    #[test]
    pub fn can_parse_issue_body() {
        let bytes = "\
This is the description.
[stuff](https://github.com/schell/repo/blob/abighash/src/File.hs#L666 \
                     \"title\")

This is another description.
[stuff](https://github.com/schell/repo/blob/abighash/src/Other.hs#L23 \
                     \"title\")

* Found on master
* Found on develop
";

        assert_eq!(
            issue_body(bytes),
            Ok((
                "",
                IssueBody {
                    descs_and_srcs: vec![
                        (
                            vec!["This is the description.".into()],
                            GitHubTodoLocation {
                                repo: ("schell".into(), "repo".into()),
                                checkout: "abighash".into(),
                                file: "src/File.hs".into(),
                                src_span: (666, None)
                            }
                        ),
                        (
                            vec!["This is another description.".into()],
                            GitHubTodoLocation {
                                repo: ("schell".into(), "repo".into()),
                                checkout: "abighash".into(),
                                file: "src/Other.hs".into(),
                                src_span: (23, None)
                            }
                        ),
                    ],
                    branches: vec!["develop".into(), "master".into(),]
                }
            ))
        );
    }
}
