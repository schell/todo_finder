//! # Parsing todos in source code.
use super::{
    langs::{CommentStyle, SupportedLanguage},
    take_to_eol,
};

use nom::{
    branch,
    bytes::complete::{self as bytes, is_not},
    character::complete as character,
    combinator,
    error::ErrorKind,
    multi, Err, IResult, Parser,
};
use std::collections::HashMap;

#[cfg(test)]
mod test_my_assumptions {
    use nom::Parser;

    use super::*;

    fn _sandbox() {
        let a: &str = "part a";
        let b: &str = "part b";
        let _: String = [a, b].join("");
    }

    #[test]
    fn trim_removes_lines() {
        let to_trim = "* Hello *\n";
        let trimmed = to_trim.trim();
        assert_eq!("* Hello *", trimmed);
    }

    #[test]
    fn trim_start_does_nothing_if_no_match() {
        let to_trim = "* Hello *\n";
        let trimmed = to_trim.trim_start_matches("!!");
        assert_eq!("* Hello *\n", trimmed);
    }

    #[test]
    fn not_eating_what_it_do() {
        let i = "blah1 blah2";
        if let Ok((i, ())) = combinator::not(todo_tag).parse(i) {
            assert_eq!(i, "blah1 blah2");
        } else {
            panic!("Failed");
        }

        let i = "TODO: blah1 blah2";
        if let Ok((_, ())) = combinator::not(todo_tag).parse(i) {
            panic!("Failed");
        }
    }

    #[test]
    fn title_and_rest() {
        let bytes = "uuid onSend/onReceive\n-- blah blah\n";
        assert_eq!(
            title_and_rest_till_eol(vec![])(bytes),
            Ok(("-- blah blah\n", ("uuid onSend/onReceive", "")))
        );

        let bytes = "This is the title! The description is less angry.\n";
        assert_eq!(
            title_and_rest_till_eol(vec![])(bytes),
            Ok(("", ("This is the title!", "The description is less angry.")))
        );

        let bytes = "This is the title?! The description is less angry.\n";
        assert_eq!(
            title_and_rest_till_eol(vec![])(bytes),
            Ok((
                "",
                ("This is the title?!", "The description is less angry.")
            ))
        );

        let bytes = "Decoder.rowVector uses '.' as an operator. So don't make a title of that.\n";
        assert_eq!(
            title_and_rest_till_eol(vec![])(bytes),
            Ok((
                "",
                (
                    "Decoder.rowVector uses '.' as an operator.",
                    "So don't make a title of that."
                )
            ))
        );
    }

    #[test]
    fn parse_single_line_todos() {
        let bytes = "-- TODO: This is a todo.\n\n\n-------------\n";
        assert_eq!(
            single_line_todo(vec![], "--".into())(bytes),
            Ok((
                "\n\n-------------\n",
                ParsedTodo::from_title("This is a todo.")
            ))
        );

        let bytes = "    # TODO: Let's have a byte to eat. Ok.\n    # TODO(): Nah, let's just \
                     have a nibble.\n    \n";
        assert_eq!(
            multi::many1(single_line_todo(vec![], "#".into())).parse(bytes),
            Ok((
                "    \n",
                vec![
                    ParsedTodo::from_title("Let's have a byte to eat.").with_desc("Ok."),
                    ParsedTodo::from_title("Nah, let's just have a nibble.")
                ]
            ))
        );

        let bytes = "    # TODO: Do A.\n    # TODO: Do B.\n";
        assert_eq!(
            single_line_todo(vec![], "#".into())(bytes),
            Ok(("    # TODO: Do B.\n", ParsedTodo::from_title("Do A.")))
        );

        let bytes = "    # TODO: aborted evaluations\n    # TODO: dependency failed without \
                     propagated builds
   for tr in d('img[alt=\"Failed\"]').parents('tr'):\n";
        assert_eq!(
            single_line_todo(vec![], "#".into())(bytes),
            Ok((
                "    # TODO: dependency failed without propagated builds
   for tr in d('img[alt=\"Failed\"]').parents('tr'):\n",
                ParsedTodo::from_title("aborted evaluations")
            ))
        );
    }

    #[test]
    fn parse_multi_line_todos() {
        let haskell_parser = multi_line_todo(vec!["|".into()], "{-".into(), "-}".into());

        let bytes = "   TODO: Make sure this comment gets turned
                          into a todo.
    -}\n";
        assert_eq!(
            haskell_parser(bytes),
            Ok((
                "\n",
                ParsedTodo {
                    assignee: None,
                    title: "Make sure this comment gets turned",
                    desc_lines: vec!["into a todo.",]
                }
            ))
        );

        let bytes = "{- | TODO: List the steps to draw an owl. -}\n";
        assert_eq!(
            haskell_parser(bytes),
            Ok((
                "",
                ParsedTodo {
                    assignee: None,
                    title: "List the steps to draw an owl.",
                    desc_lines: vec![]
                }
            ))
        );

        let bytes = "{- TODO: Figure out why duplicate tickets are being made.
          The todo above \"Add log levels\" is getting re-created on each check-in.

          Fix dis shizz!
       -}\n";

        assert_eq!(
            haskell_parser(bytes),
            Ok((
                "\n",
                ParsedTodo {
                    assignee: None,
                    title: "Figure out why duplicate tickets are being made.",
                    desc_lines: vec![
                        "The todo above \"Add log levels\" is getting re-created on each check-in.",
                        "Fix dis shizz!"
                    ]
                }
            ))
        );
    }

    #[test]
    fn parse_todos() {
        let c_parser = parse_todo(TodoParserConfig {
            singles: vec!["//".into()],
            multis: vec![("/*".into(), "*/".into())],
            borders: vec!["*".into()],
        });

        let bytes = "/** FIXME: C++ doc title.
        * C++ doc body. Here is some detail
        * that is really interesting.
        */\n";
        assert_eq!(
            c_parser(bytes),
            Ok((
                "\n",
                ParsedTodo {
                    title: "C++ doc title.",
                    assignee: None,
                    desc_lines: vec![
                        "C++ doc body. Here is some detail",
                        "that is really interesting."
                    ]
                }
            ))
        );

        let nix_parser = parse_todo(TodoParserConfig {
            singles: vec!["#".into()],
            multis: vec![],
            borders: vec![],
        });

        let bytes = "    # TODO: aborted evaluations\n    # TODO: dependency failed without \
                     propagated builds\n    for tr in d('img[alt=\"Failed\"]').parents('tr'):\n";
        assert_eq!(
            nix_parser(bytes),
            Ok((
                "    # TODO: dependency failed without propagated builds\n    for tr in \
                 d('img[alt=\"Failed\"]').parents('tr'):\n",
                ParsedTodo {
                    title: "aborted evaluations",
                    assignee: None,
                    desc_lines: vec![]
                }
            ))
        );
    }
}

/// Eat a single or multi line comment start.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// let (borders, single) = (vec!["|".to_string()], "--".to_string());
///
/// assert_eq!(
///     comment_start(borders, single)(" -- Here is a comment."),
///     Ok(("Here is a comment.", ()))
/// );
/// ```
pub fn comment_start(
    // An ignorable border for comments that like to have outlines.
    // Eg. "*" for C-like langs or "!" for Objective-C.
    borders: Vec<String>,
    // The comment prefix.
    // Eg. "--" or "{-" for Haskell, "//" or "/*" for Rust.
    prefix: String,
) -> impl Fn(&str) -> IResult<&str, ()> {
    move |i: &str| {
        let (i, _) = character::space0(i)?;
        let (i, _) = bytes::tag(prefix.as_str())(i)?;
        let (i, _) = character::space0(i)?;
        let i = {
            let mut input_left = i;
            'eat_borders: for border in borders.iter() {
                let (input, ate) =
                    combinator::opt(bytes::tag(border.as_str())).parse(input_left)?;
                input_left = input;
                if ate.is_some() {
                    break 'eat_borders;
                }
            }
            input_left
        };
        let (i, _) = character::space0(i)?;
        Ok((i, ()))
    }
}

/// Eat an assigned name.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// assert_eq!(assignee("(mitchellwrosen)"), Ok(("", "mitchellwrosen")))
/// ```
pub fn assignee(i: &str) -> IResult<&str, &str> {
    let (i, _) = character::char('(')(i)?;
    let (i, _) = character::space0(i)?;
    let is_end = |input: char| input != '\r' && input != '\n' && input != ' ' && input != ')';
    let (i, name) = bytes::take_while(is_end)(i)?;
    let (i, _) = character::char(')')(i)?;
    Ok((i, name))
}

/// Patterns that denote a TODO.
pub const TAG_PATTERNS: &[&str; 4] = &["TODO", "FIXME", "@todo", "todo!"];

/// The start of a TODO.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TodoTag<'a> {
    Standard(&'a str),
    RustMacro,
}

/// Eat a todo tag. Currently supports `TODO`, `FIXME`, `@todo` and `todo!`.
/// It will also eat and return any assigned name following the todo tag, with
/// the exception of a `todo!`, which contains the title instead of an assignee.
///
/// ```rust
/// use nom::{multi, Parser};
/// use todo_finder_lib::parser::source::*;
///
/// assert_eq!(todo_tag("@todo "), Ok(("", None)));
/// assert_eq!(todo_tag("TODO "), Ok(("", None)));
/// assert_eq!(todo_tag("TODO"), Ok(("", None)));
/// assert_eq!(todo_tag("FIXME"), Ok(("", None)));
/// assert_eq!(todo_tag("todo!"), Ok(("", Some(TodoTag::RustMacro))));
///
/// let all_text = r#"TODO(schell) FIXME (mitchellwrosen) @todo(imalsogreg) todo!("blah")"#;
/// let parsed = multi::many1(|i| todo_tag(i)).parse(all_text);
/// assert_eq!(
///     parsed,
///     Ok((
///         r#"("blah")"#,
///         vec![
///             Some(TodoTag::Standard("schell")),
///             Some(TodoTag::Standard("mitchellwrosen")),
///             Some(TodoTag::Standard("imalsogreg")),
///             Some(TodoTag::RustMacro)
///         ]
///     ))
/// );
/// ```
pub fn todo_tag(i: &'_ str) -> IResult<&'_ str, Option<TodoTag<'_>>> {
    let (i, _) = character::space0(i)?;
    let [todo, fixme, at_todo, rust_todo] = TAG_PATTERNS;
    let tags = (
        bytes::tag(*todo),
        bytes::tag(*fixme),
        bytes::tag(*at_todo),
        bytes::tag(*rust_todo),
    );
    let (i, tag) = branch::alt(tags).parse(i)?;
    if &tag == rust_todo {
        return Ok((i, Some(TodoTag::RustMacro)));
    }

    let (i, _) = character::space0(i)?;
    let (i, may_name) = combinator::opt(|i| assignee(i)).parse(i)?;
    let (i, _) = character::space0(i)?;
    let (i, _) = combinator::opt(character::char(':')).parse(i)?;
    let (i, _) = character::space0(i)?;
    Ok((i, may_name.map(TodoTag::Standard)))
}

/// Eat a sentence and its terminator and a space.
/// Terminators must have an empty space after them to be considered valid,
/// otherwise they could be a programming operator.
/// The entire eaten sentence and terminators will be returned in a Vector of
/// slices.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// let bytes = "Sandy doesn't like fmap.fmap for some reason. Other sentence.\n";
/// assert_eq!(
///     sentence_and_terminator(bytes),
///     Ok((
///         "Other sentence.\n",
///         "Sandy doesn't like fmap.fmap for some reason."
///     ))
/// );
///
/// let bytes = "Hey you, I bet you allocate\na lot of resources. Maybe not.";
/// assert_eq!(
///     sentence_and_terminator(bytes),
///     Ok((
///         "Maybe not.",
///         "Hey you, I bet you allocate\na lot of resources."
///     ))
/// );
/// ```
pub fn sentence_and_terminator(i: &str) -> IResult<&str, &str> {
    let is_terminator = |c: char| c == '.' || c == '?' || c == '!';
    let mut n = 0;
    let mut ii = i;
    'eating_sentences: loop {
        let (j, sentence) = bytes::take_till(is_terminator)(ii)?;
        let (j, terminators) = bytes::take_while(is_terminator)(j)?;
        let (j, space) = combinator::opt(character::char(' ')).parse(j)?;
        ii = j;
        n += sentence.len();
        n += terminators.len();
        if space.is_some() || j.is_empty() {
            // Unless we get a space or are at the end, keep eating more
            break 'eating_sentences;
        }
    }
    let (sentence, i) = i.split_at(n);
    let i = i.trim_start();
    Ok((i, sentence))
}

/// Trim any source code borders off of the string. This only accounts for
/// borders on the beginning and end of the string, not in the middle. To remove
/// borders from the middle of a string, first break it into lines. Furthermore
/// this function *will remove trailing whitespace, including line breaks*.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// assert_eq!(
///     trim_borders(&vec!["*".into()], " * I like veggies *\n"),
///     "I like veggies"
/// );
/// ```
pub fn trim_borders<'a>(borders: &[String], i: &'a str) -> &'a str {
    let i = i.trim();
    let i = borders
        .iter()
        .fold(i, |i, border| i.trim_start_matches(border).trim());
    borders
        .iter()
        .fold(i, |i, border| i.trim_end_matches(border).trim())
}

/// Eat a sentence and the rest of the line, if possible. The rest, in the case
/// of a todo, is a portion of the description.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// let bytes = "sleep for variable time depending on exact error? Ionno know.\n\n";
/// assert_eq!(
///     title_and_rest_till_eol(vec![])(bytes),
///     Ok((
///         "\n",
///         (
///             "sleep for variable time depending on exact error?",
///             "Ionno know."
///         )
///     ))
/// );
/// ```
pub fn title_and_rest_till_eol(
    // An ignorable border for comments that like to have outlines.
    // Eg. "*" for C-like langs or "!" for Objective-C.
    borders: Vec<String>,
) -> impl Fn(&str) -> IResult<&str, (&str, &str)> {
    move |i| {
        let (i, ln) = take_to_eol(i)?;
        let (desc, title) = sentence_and_terminator(ln)?;
        Ok((i, (title, trim_borders(&borders, desc))))
    }
}

/// Eat a single line comment. Fails if it hits a possible todo and stops when it
/// eats the end of a line.
///
/// ```rust
/// use nom::CompareResult::Error;
/// use todo_finder_lib::parser::source::*;
///
/// let bytes = "// Here is a whole single line comment.\n";
/// assert_eq!(
///     single_line_comment(vec![], "//".into())(bytes),
///     Ok(("", "Here is a whole single line comment."))
/// );
///
/// let bytes = "// TODO: Here is a whole single line comment.\n";
/// assert!(single_line_comment(vec![], "//".into())(bytes).is_err());
/// ```
pub fn single_line_comment(
    // An ignorable border for comments that like to have outlines.
    // Eg. "*" for C-like langs or "!" for Objective-C.
    borders: Vec<String>,
    // The comment prefix.
    // Eg. "--" for Haskell, "//" for Rust.
    prefix: String,
) -> impl Fn(&str) -> IResult<&str, &str> {
    let parse_comment_start = comment_start(borders, prefix);
    move |i| {
        let (i, _) = parse_comment_start(i)?;
        let (i, _) = combinator::not(todo_tag).parse(i)?;
        take_to_eol(i)
    }
}

/// Eat a todo comprised of single line comments.
/// Returns an assignee if possible, the todo's title and a vector of description
/// lines.
///
/// ```rust
/// use nom::multi;
/// use todo_finder_lib::parser::source::*;
///
/// let bytes = "-- TODO: Hey there.\n--    Description.\n";
/// assert_eq!(
///     single_line_todo(vec![], "--".into())(bytes),
///     Ok(("", ParsedTodo::from_title("Hey there.").with_desc("Description.")))
/// );
/// ```
#[allow(clippy::type_complexity)]
pub fn single_line_todo(
    // An ignorable border for comments that like to have outlines.
    // Eg. "*" for C-like langs or "!" for Objective-C.
    borders: Vec<String>,
    // The comment prefix.
    // Eg. "--" for Haskell, "//" for Rust.
    prefix: String,
) -> impl Fn(&str) -> IResult<&str, ParsedTodo> {
    let parse_comment_start = comment_start(borders.clone(), prefix.clone());
    let parse_title_desc = title_and_rest_till_eol(borders.clone());
    move |i| {
        let (i, _) = parse_comment_start(i)?;
        let (i, may_name) = todo_tag(i)?;
        let may_name = match may_name {
            Some(TodoTag::Standard(name)) => {
                if name.is_empty() {
                    None
                } else {
                    Some(name)
                }
            }
            Some(TodoTag::RustMacro) => {
                return rust_todo_content(i);
            }
            None => None,
        };
        let (i, (title, desc0)) = parse_title_desc(i)?;
        let parse_single_line = single_line_comment(borders.clone(), prefix.clone());
        let (i, mut desc_n) = multi::many0(parse_single_line).parse(i)?;
        desc_n.insert(0, desc0);
        desc_n.retain(|desc| !desc.is_empty());
        Ok((
            i,
            ParsedTodo {
                assignee: may_name,
                title,
                desc_lines: desc_n,
            },
        ))
    }
}

pub fn rust_todo_content(i: &'_ str) -> IResult<&'_ str, ParsedTodo<'_>> {
    let (i, content) =
        nom::sequence::delimited(character::char('('), is_not(")"), character::char(')'))
            .parse(i)?;
    let content = content.trim();
    let content = content.trim_matches('"');

    let parse_title_desc = title_and_rest_till_eol(vec![]);
    let (mut rest, (title, desc)) = parse_title_desc(content)?;
    let mut desc_lines = vec![];
    if !desc.is_empty() {
        desc_lines.push(desc.trim().trim_end_matches("\\").trim());
    }
    while !rest.is_empty() {
        let (next_rest, line) = take_to_eol(rest)?;
        rest = next_rest;
        if !line.is_empty() {
            desc_lines.push(line.trim().trim_end_matches("\\").trim());
        }
    }
    Ok((
        i,
        ParsedTodo {
            assignee: None,
            title: title.trim().trim_end_matches("\\").trim(),
            desc_lines,
        },
    ))
}

/// Eat a todo that lives in a multi-line comment block.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// let haskell_parser = multi_line_todo(vec!["|".into()], "{-".into(), "-}".into());
///
/// let bytes = "{- | TODO: My todo title.
///                   Description too. With more
///                   sentences over more lines.
///              -}\n";
/// assert_eq!(
///     haskell_parser(bytes),
///     Ok((
///         "\n",
///         ParsedTodo::from_title("My todo title.")
///           .with_desc("Description too. With more")
///           .with_desc("sentences over more lines.")
///     ))
/// );
/// ```
#[allow(clippy::type_complexity)]
pub fn multi_line_todo(
    // An ignorable border for comments that like to have outlines.
    // Eg. "*" for C-like langs or "!" for Objective-C.
    borders: Vec<String>,
    // The comment prefix.
    // Eg. "{-" for Haskell, "/*" for Rust.
    prefix: String,
    // The comment suffix.
    // Eg. "-}" for Haskell, "*/" for Rust.
    suffix: String,
) -> impl Fn(&str) -> IResult<&str, ParsedTodo> {
    let parse_title_desc = title_and_rest_till_eol(borders.clone());
    move |i| {
        let (i, _) = character::space0(i)?;
        let (i, _) = combinator::opt(comment_start(borders.clone(), prefix.clone())).parse(i)?;
        let (i, may_name) = todo_tag(i)?;
        let may_name = match may_name {
            None => None,
            Some(TodoTag::Standard(name)) => Some(name),
            Some(TodoTag::RustMacro) => {
                return rust_todo_content(i);
            }
        };
        let (i, (title, desc0)) = parse_title_desc(i)?;
        if desc0 == suffix {
            Ok((
                i,
                ParsedTodo {
                    assignee: may_name,
                    title,
                    desc_lines: vec![],
                },
            ))
        } else {
            let (i, comment) = bytes::take_until(suffix.as_str())(i)?;
            let (i, _) = bytes::tag(suffix.as_str())(i)?;
            let mut desc_n = vec![desc0];
            for line in comment.lines() {
                let trimmed_line = trim_borders(&borders, line);
                desc_n.push(trimmed_line);
            }
            desc_n.retain(|desc| !desc.is_empty());
            Ok((
                i,
                ParsedTodo {
                    assignee: may_name,
                    title,
                    desc_lines: desc_n,
                },
            ))
        }
    }
}

/// A todo parser configuration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TodoParserConfig {
    /// A list of single comment openers.
    /// Eg. `vec!["--".into()]` for Haskell
    pub singles: Vec<String>,
    /// A list of multiline comment openers and closers.
    /// Eg. `vec![("{-".into(), "-}".into())]` for Haskell
    pub multis: Vec<(String, String)>,
    /// A list of comment borders.
    /// Eg. `vec!["|".into()]` for Haskell
    pub borders: Vec<String>,
}

impl TodoParserConfig {
    pub fn add_comment_style(&mut self, cs: CommentStyle) {
        match cs {
            CommentStyle::Single(s) => {
                self.singles.push(s);
            }
            CommentStyle::Multi(p, s) => {
                self.multis.push((p, s));
            }
            CommentStyle::Border(b) => self.borders.push(b),
        }
    }

    pub fn from_comment_styles(styles: Vec<CommentStyle>) -> Self {
        let mut cfg = TodoParserConfig::default();
        styles
            .into_iter()
            .for_each(|style| cfg.add_comment_style(style));
        cfg
    }

    pub fn add_parser_config(&mut self, cfg: TodoParserConfig) {
        self.singles.extend(cfg.singles);
        self.multis.extend(cfg.multis);
        self.borders.extend(cfg.borders);
    }
}

#[derive(Default)]
pub struct ParserConfigLookup(pub HashMap<String, TodoParserConfig>);

impl ParserConfigLookup {
    pub fn add_lang(&mut self, language: SupportedLanguage) {
        let cfg = TodoParserConfig::from_comment_styles(language.comment_styles);
        for ext in language.file_extensions {
            let old_cfg = self.0.entry(ext).or_default();
            old_cfg.add_parser_config(cfg.clone());
        }
    }

    pub fn find_parser_config(&self, ext: String) -> Option<&TodoParserConfig> {
        let ext = ext.to_lowercase();
        self.0.get(&ext)
    }
}

/// A structure to conveniently hold a fully parsed todo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedTodo<'a> {
    pub title: &'a str,
    pub assignee: Option<&'a str>,
    pub desc_lines: Vec<&'a str>,
}

impl<'a> ParsedTodo<'a> {
    pub fn from_title(title: &'a str) -> Self {
        ParsedTodo {
            title,
            ..Default::default()
        }
    }

    pub fn with_desc(mut self, line: &'a str) -> Self {
        self.desc_lines.push(line);
        self
    }
}

/// Configures a parser to eat a todo from the input.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;
///
/// let haskell_parser = parse_todo(TodoParserConfig {
///     singles: vec!["--".into()],
///     multis: vec![("{-".into(), "-}".into())],
///     borders: vec!["|".into()],
/// });
///
/// let bytes = "{- | TODO (soundwave) List the steps to draw an owl. -}\n";
/// assert_eq!(
///     haskell_parser(bytes),
///     Ok((
///         "",
///         ParsedTodo {
///             title: "List the steps to draw an owl.",
///             assignee: Some("soundwave"),
///             desc_lines: vec![]
///         }
///     ))
/// );
/// ```
pub fn parse_todo<'a>(
    cfg: TodoParserConfig,
) -> impl Fn(&'a str) -> IResult<&'a str, ParsedTodo<'a>> {
    move |i| {
        for (prefix, suffix) in cfg.multis.clone() {
            if let Ok(res) = multi_line_todo(cfg.borders.clone(), prefix, suffix)(i) {
                return Ok(res);
            }
        }

        for prefix in cfg.singles.clone() {
            if let Ok(res) = single_line_todo(cfg.borders.clone(), prefix)(i) {
                return Ok(res);
            }
        }

        // Lastly, try a plain

        Err(Err::Error(nom::error::Error {
            input: i,
            code: ErrorKind::Tag,
        }))
    }
}

/// Using the given config, return a parser that will parse any and all todos
/// from the string.
pub fn parse_todos<'a>(cfg: TodoParserConfig) -> impl FnMut(&'a str) -> Vec<ParsedTodo<'a>> {
    let mut parser = multi::many_till(take_to_eol, parse_todo(cfg));
    move |i: &str| {
        let mut todos = vec![];
        let mut ii = i;

        'find: loop {
            if ii.is_empty() {
                break 'find;
            }
            if let Ok((j, (_, todo))) = parser.parse(ii) {
                ii = j;
                todos.push(todo);
            } else {
                break 'find;
            }
        }

        todos
    }
}
