//! # Parsing todos in source code.
use super::{
    langs::{CommentStyle, SupportedLanguage},
    take_to_eol,
};

use nom::{
    branch, bytes::complete as bytes, character::complete as character, combinator,
    error::ErrorKind, multi, Err, IResult,
};
use std::collections::HashMap;

#[cfg(test)]
mod test_my_assumptions {
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
        if let Ok((i, ())) = combinator::not(todo_tag)(i) {
            assert_eq!(i, "blah1 blah2");
        } else {
            assert!(false, "Failed");
        }

        let i = "TODO: blah1 blah2";
        if let Ok((_, ())) = combinator::not(todo_tag)(i) {
            assert!(false, "Failed");
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
            Ok(("\n\n-------------\n", (None, "This is a todo.", vec![])))
        );

        let bytes = "    # TODO: Let's have a byte to eat. Ok.\n    # TODO(): Nah, let's just \
                     have a nibble.\n    \n";
        assert_eq!(
            multi::many1(single_line_todo(vec![], "#".into()))(bytes),
            Ok((
                "    \n",
                vec![
                    (None, "Let's have a byte to eat.", vec!["Ok.".into()]),
                    (Some(""), "Nah, let's just have a nibble.", vec![])
                ]
            ))
        );

        let bytes = "    # TODO: Do A.\n    # TODO: Do B.\n";
        assert_eq!(
            single_line_todo(vec![], "#".into())(bytes),
            Ok(("    # TODO: Do B.\n", (None, "Do A.", vec![])))
        );

        let bytes = "    # TODO: aborted evaluations\n    # TODO: dependency failed without \
                     propagated builds
   for tr in d('img[alt=\"Failed\"]').parents('tr'):\n";
        assert_eq!(
            single_line_todo(vec![], "#".into())(bytes),
            Ok((
                "    # TODO: dependency failed without propagated builds
   for tr in d('img[alt=\"Failed\"]').parents('tr'):\n",
                (None, "aborted evaluations", vec![])
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
                (
                    None,
                    "Make sure this comment gets turned",
                    vec!["into a todo.",]
                )
            ))
        );

        let bytes = "{- | TODO: List the steps to draw an owl. -}\n";
        assert_eq!(
            haskell_parser(bytes),
            Ok(("", (None, "List the steps to draw an owl.", vec![])))
        );

        let bytes = "{- TODO: Figure out why duplicate tickets are being made.
          The todo above \"Add log levels\" is getting re-created on each check-in.

          Fix dis shizz!
       -}\n";

        assert_eq!(
            haskell_parser(bytes),
            Ok((
                "\n",
                (
                    None,
                    "Figure out why duplicate tickets are being made.",
                    vec![
                        "The todo above \"Add log levels\" is getting re-created on each check-in.",
                        "Fix dis shizz!"
                    ]
                )
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
/// use todo_finder_lib::parser::source::*;;
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
                let (input, ate) = combinator::opt(bytes::tag(border.as_str()))(input_left)?;
                input_left = input;
                match ate {
                    Some(_) => {
                        break 'eat_borders;
                    }
                    None => {}
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
/// use todo_finder_lib::parser::source::*;;
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

/// Eat a todo tag. Currently supports `TODO`, `FIXME` and `@todo`.
/// It will also eat any assigned name following the todo tag and return it.
///
/// ```rust
/// use nom::multi;
/// use todo_finder_lib::parser::source::*;
///
/// assert_eq!(todo_tag("@todo "), Ok(("", None)));
/// assert_eq!(todo_tag("TODO "), Ok(("", None)));
/// assert_eq!(todo_tag("TODO"), Ok(("", None)));
/// assert_eq!(todo_tag("FIXME"), Ok(("", None)));
///
/// let all_text = "TODO(schell) FIXME (mitchellwrosen) @todo(imalsogreg)";
/// let parsed = multi::many1(|i| todo_tag(i))(all_text);
/// assert_eq!(
///     parsed,
///     Ok((
///         "",
///         vec![Some("schell"), Some("mitchellwrosen"), Some("imalsogreg")]
///     ))
/// );
/// ```
pub fn todo_tag(i: &str) -> IResult<&str, Option<&str>> {
    let (i, _) = character::space0(i)?;
    let tags = (bytes::tag("TODO"), bytes::tag("FIXME"), bytes::tag("@todo"));
    let (i, _) = branch::alt(tags)(i)?;
    let (i, _) = character::space0(i)?;
    let (i, may_name) = combinator::opt(|i| assignee(i))(i)?;
    let (i, _) = character::space0(i)?;
    let (i, _) = combinator::opt(character::char(':'))(i)?;
    let (i, _) = character::space0(i)?;
    Ok((i, may_name))
}

/// Eat a sentence and its terminator and a space.
/// Terminators must have an empty space after them to be considered valid,
/// otherwise they could be a programming operator.
/// The entire eaten sentence and terminators will be returned in a Vector of
/// slices.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;;
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
        let (j, space) = combinator::opt(character::char(' '))(j)?;
        ii = j;
        n += sentence.len();
        n += terminators.len();
        if space.is_some() || j == "" {
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
/// use todo_finder_lib::parser::source::*;;
///
/// assert_eq!(
///     trim_borders(&vec!["*".into()], " * I like veggies *\n"),
///     "I like veggies"
/// );
/// ```
pub fn trim_borders<'a>(borders: &Vec<String>, i: &'a str) -> &'a str {
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
/// use todo_finder_lib::parser::source::*;;
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
        let (i, _) = combinator::not(todo_tag)(i)?;
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
///     Ok(("", (None, "Hey there.", vec!["Description.".into()])))
/// );
/// ```
pub fn single_line_todo(
    // An ignorable border for comments that like to have outlines.
    // Eg. "*" for C-like langs or "!" for Objective-C.
    borders: Vec<String>,
    // The comment prefix.
    // Eg. "--" for Haskell, "//" for Rust.
    prefix: String,
) -> impl Fn(&str) -> IResult<&str, (Option<&str>, &str, Vec<&str>)> {
    let parse_comment_start = comment_start(borders.clone(), prefix.clone());
    let parse_title_desc = title_and_rest_till_eol(borders.clone());
    move |i| {
        let (i, _) = parse_comment_start(i)?;
        let (i, may_name) = todo_tag(i)?;
        let (i, (title, desc0)) = parse_title_desc(i)?;
        let parse_single_line = single_line_comment(borders.clone(), prefix.clone());
        let (i, mut desc_n) = multi::many0(parse_single_line)(i)?;
        desc_n.insert(0, desc0);
        desc_n.retain(|desc| !desc.is_empty());
        Ok((i, (may_name, title, desc_n)))
    }
}

/// Eat a todo that lives in a multi-line comment block.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;;
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
///         (
///             None,
///             "My todo title.",
///             vec!["Description too. With more", "sentences over more lines."]
///         )
///     ))
/// );
/// ```
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
) -> impl Fn(&str) -> IResult<&str, (Option<&str>, &str, Vec<&str>)> {
    let parse_title_desc = title_and_rest_till_eol(borders.clone());
    move |i| {
        let (i, _) = character::space0(i)?;
        let (i, _) = combinator::opt(comment_start(borders.clone(), prefix.clone()))(i)?;
        let (i, may_name) = todo_tag(i)?;
        let (i, (title, desc0)) = parse_title_desc(i)?;
        if desc0 == &suffix {
            Ok((i, (may_name, title, vec![])))
        } else {
            let (i, comment) = bytes::take_until(suffix.as_str())(i)?;
            let (i, _) = bytes::tag(suffix.as_str())(i)?;
            let mut desc_n = vec![desc0];
            for line in comment.lines() {
                let trimmed_line = trim_borders(&borders, line);
                desc_n.push(trimmed_line);
            }
            desc_n.retain(|desc| !desc.is_empty());
            Ok((i, (may_name, title, desc_n)))
        }
    }
}

/// A todo parser configuration.
#[derive(Clone, Debug, PartialEq)]
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
    pub fn new() -> Self {
        TodoParserConfig {
            singles: vec![],
            multis: vec![],
            borders: vec![],
        }
    }

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
        let mut cfg = TodoParserConfig::new();
        styles
            .into_iter()
            .for_each(|style| cfg.add_comment_style(style));
        cfg
    }

    pub fn add_parser_config(&mut self, cfg: TodoParserConfig) {
        self.singles.extend(cfg.singles.into_iter());
        self.multis.extend(cfg.multis.into_iter());
        self.borders.extend(cfg.borders.into_iter());
    }
}

pub struct ParserConfigLookup(pub HashMap<String, TodoParserConfig>);

impl ParserConfigLookup {
    pub fn new() -> Self {
        ParserConfigLookup(HashMap::new())
    }

    pub fn add_lang(&mut self, language: SupportedLanguage) {
        let cfg = TodoParserConfig::from_comment_styles(language.comment_styles);
        for ext in language.file_extensions {
            let old_cfg = self.0.entry(ext).or_insert(TodoParserConfig::new());
            old_cfg.add_parser_config(cfg.clone());
        }
    }

    pub fn find_parser_config(&self, ext: String) -> Option<&TodoParserConfig> {
        let ext = ext.to_lowercase();
        self.0.get(&ext)
    }
}

/// A structure to conveniently hold a fully parsed todo.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedTodo<'a> {
    pub title: &'a str,
    pub assignee: Option<&'a str>,
    pub desc_lines: Vec<&'a str>,
}

/// Configures a parser to eat a todo from the input.
///
/// ```rust
/// use todo_finder_lib::parser::source::*;;
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
        let to_todo = |(input, todo): (&'a str, (Option<&'a str>, &'a str, Vec<&'a str>))| {
            Ok((
                input,
                ParsedTodo {
                    title: todo.1,
                    assignee: todo.0,
                    desc_lines: todo.2,
                },
            ))
        };

        for (prefix, suffix) in cfg.multis.clone() {
            let res = multi_line_todo(cfg.borders.clone(), prefix, suffix)(i);
            if let Ok(res) = res {
                return to_todo(res);
            }
        }

        for prefix in cfg.singles.clone() {
            let res = single_line_todo(cfg.borders.clone(), prefix)(i);
            if let Ok(res) = res {
                return to_todo(res);
            }
        }

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
            if let Ok((j, (_, todo))) = parser(ii) {
                ii = j;
                todos.push(todo);
            } else {
                break 'find;
            }
        }

        todos
    }
}
