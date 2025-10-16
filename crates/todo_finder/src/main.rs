use clap::Parser;
use std::{fs::File, io::prelude::*, path::Path};
use todo_finder_lib::{github, parser::IssueMap};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum IssueProvider {
    Github,
    Markdown,
}

// impl core::fmt::Display for IssueProvider {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.write_str(match self {
//             IssueProvider::Github => "github",
//             IssueProvider::Markdown => "markdown",
//         })
//     }
// }

// impl FromStr for IssueProvider {
//     type Err = String;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(match s {
//             "github" => Self::Github,
//             "markdown" => Self::Markdown,
//             s => return Err(format!("{s} is not a supported issue provider")),
//         })
//     }
// }

#[derive(clap::Parser, Debug)]
#[command(
    version,
    about,
    author,
    help_template(
        "\
        {before-help}{name} {version} by {author-with-newline}
        {about-with-newline}
        {usage-heading} {usage}
        
        {all-args}{after-help}"
    )
)]
struct Cli {
    #[clap(long = "issue_provider", short, default_value("markdown"))]
    /// The issue provider, eg GitHub or "markdown" for a local file
    output: IssueProvider,

    #[clap(short, long)]
    /// An authorization token, depending on the issue provider.
    auth: Option<String>,

    #[clap(short, long, default_value = "todo")]
    /// Label to apply to all created TODOs at the issue provider.
    label: String,

    /// Regular expression of files or directories to ignore,
    /// may be supplied multiple times.
    #[clap(short, long)]
    exclude: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cwd = std::env::current_dir().expect("could not get current dir");
    let cwd_str = cwd.to_str().expect("could not convert cwd path");

    let Cli {
        output,
        auth,
        label,
        exclude,
    } = Cli::parse();

    match output {
        IssueProvider::Markdown => {
            let file_name = "todos.md";
            let issues = IssueMap::from_files_in_directory(cwd_str, &exclude).unwrap();
            let markdown = issues.as_markdown();
            let path = Path::new(file_name);
            let mut file = File::create(path)
                .unwrap_or_else(|_| panic!("could not create file {}", file_name));
            let bytes = markdown.as_bytes();
            file.write_all(bytes)
                .unwrap_or_else(|_| panic!("could not write to file {}", file_name));
            println!("TODOs written to {:#?}", path);
        }

        IssueProvider::Github => {
            let auth_token = auth.expect("github requires an auth");
            github::run_ts_github(auth_token, label, cwd_str.into(), &exclude)
                .await
                .unwrap();
        }
    }
}
