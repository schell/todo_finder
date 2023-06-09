use clap::{App, Arg};
use std::{fs::File, io::prelude::*, path::Path};
use todo_finder_lib::{github, parser::IssueMap};

#[tokio::main]
async fn main() {
    let cwd = std::env::current_dir().expect("could not get current dir");
    let cwd_str = cwd.to_str().expect("could not convert cwd path");

    let app = App::new("todo_finder")
        .version("0.1.0")
        .author("Schell Carl Scivally")
        .about("Finds TODOs in source code")
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("issue_provider")
                .value_name("PROVIDER")
                .help("One of 'markdown' or 'github'")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("auth")
                .short("a")
                .long("auth")
                .value_name("AUTHORIZATION")
                .help("Depending on the value of --output, an authorization token")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("label")
                .short("l")
                .long("label")
                .value_name("ISSUE_LABEL")
                .help("Label to apply to all created TODOs at the issue provider")
                .default_value("todo")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("exclude")
                .short("e")
                .long("exclude")
                .value_name("PATTERN")
                .help("Regex of files or directories to ignore, may be supplied multiple times")
                .multiple(true)
                .takes_value(true),
        );

    let matches = app.get_matches();
    let exclusions: Vec<String> = matches
        .value_of("exclude")
        .map(|s| s.split(" ").map(|s| s.to_string()).collect::<Vec<_>>())
        .unwrap_or(vec![]);

    match matches.value_of("output").expect("--output required") {
        "markdown" => {
            let file_name = "todos.md";
            let issues = IssueMap::from_files_in_directory(cwd_str, &exclusions).unwrap();
            let markdown = issues.as_markdown();
            let path = Path::new(file_name);
            let mut file =
                File::create(path).expect(&format!("could not create file {}", file_name));
            let bytes = markdown.as_bytes();
            file.write_all(bytes)
                .expect(&format!("could not write to file {}", file_name));
            println!("TODOs written to {:#?}", path);
        }

        "github" => {
            let auth_token = matches.value_of("auth").expect("github requires an auth");
            let issue_label = matches
                .value_of("label")
                .expect("github requires an issue label");
            github::run_ts_github(
                auth_token.into(),
                issue_label.into(),
                cwd_str.into(),
                &exclusions,
            )
            .await
            .unwrap();
        }

        _ => panic!("invalid value for 'output'"),
    }
}
