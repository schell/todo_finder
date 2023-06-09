use nom::{
    branch, bytes::complete as bytes, character::complete as character, combinator, multi, IResult,
};

pub fn parse_owner_and_repo_from_config(i: &str) -> IResult<&str, (&str, &str)> {
    let (i, (owner, repo)) = branch::alt((
        parse_owner_and_repo_from_config_git,
        parse_owner_and_repo_from_config_http,
    ))(i)?;
    Ok((i, (owner.trim(), repo.trim())))
}

pub fn parse_owner_and_repo_from_config_git(i: &str) -> IResult<&str, (&str, &str)> {
    let (i, _) = bytes::tag("git@")(i)?;
    let (i, _) = bytes::take_till(|c| c == ':')(i)?;
    let (i, _) = character::char(':')(i)?;
    let (i, owner) = bytes::take_till(|c| c == '/')(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, repo) = bytes::take_till(|c| c == '.')(i)?;
    Ok((i, (owner, repo)))
}

pub fn parse_owner_and_repo_from_config_http(i: &str) -> IResult<&str, (&str, &str)> {
    let (i, _) = bytes::tag("http")(i)?;
    let (i, _) = combinator::opt(character::char('s'))(i)?;
    let (i, _) = bytes::tag("://")(i)?;
    let (i, _) = bytes::take_till(|c| c == '/')(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, owner) = bytes::take_till(|c| c == '/')(i)?;
    let (i, _) = character::char('/')(i)?;
    let (i, repo) = bytes::take_till(|c| c == '.')(i)?;
    Ok((i, (owner, repo)))
}

/// Eat a whole line and optionally its ending but don't return that ending.
pub fn take_to_eol(i: &str) -> IResult<&str, &str> {
    let (i, ln) = bytes::take_till(|c| c == '\r' || c == '\n')(i)?;
    let (i, _) = combinator::opt(character::line_ending)(i)?;
    Ok((i, ln))
}

pub fn parse_rg_line(i: &str) -> IResult<&str, usize> {
    let (i, lnum) = character::digit1(i)?;
    let (i, _) = character::char(':')(i)?;
    let (i, _) = take_to_eol(i)?;
    let lnum: usize = lnum.parse().expect("line number is not a number");
    Ok((i, lnum))
}

pub fn parse_rg_file(i: &str) -> IResult<&str, (&str, Vec<usize>)> {
    let (i, file) = take_to_eol(i)?;
    let (i, line_nums) = multi::many1(parse_rg_line)(i)?;
    let (i, _) = combinator::opt(character::line_ending)(i)?;
    Ok((i, (file, line_nums)))
}

pub fn parse_rg(i: &str) -> IResult<&str, Vec<(&str, Vec<usize>)>> {
    multi::many1(parse_rg_file)(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT:&'static [u8] = b"\
test_data/two.rs
1:This is another test file. The following is some garbage from my dayjob, with TODO tags sprinkled in.
13:// TODO: Here is an actual todo.
15:// TODO: Here is an actual todo.
32:/// TODO: Last line todo title.

test_data/one.rs
1:This is a test file. The following is some garbage from my dayjob, with TODO tags sprinkled in.
13:// TODO: Here is an actual todo.
30:/// TODO: Another todo.
";

    #[test]
    fn can_parse_rg_output() {
        let rg_output = std::str::from_utf8(OUTPUT).expect("Could not convert output");
        let res = parse_rg(rg_output);
        assert!(res.is_ok());
        let (_, files) = res.unwrap();
        assert_eq!(
            files,
            vec![
                ("test_data/two.rs", vec![1, 13, 15, 32]),
                ("test_data/one.rs", vec![1, 13, 30])
            ]
        );
    }

    #[test]
    fn can_parse_git_config_owner_repo() {
        assert_eq!(
            parse_owner_and_repo_from_config("git@github.com:schell/todo_sync.git"),
            Ok((".git", ("schell", "todo_sync")))
        );

        assert_eq!(
            parse_owner_and_repo_from_config("https://github.com/schell/todo_sync"),
            Ok(("", ("schell", "todo_sync")))
        );
    }
}
