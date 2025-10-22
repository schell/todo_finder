use clap::Parser;
use console::Style;
use futures::FutureExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use todo_finder_lib::{github, parser::IssueMap, Message};

#[derive(Debug, Default, Clone, clap::Parser)]
struct GitHubProvider {
    #[clap(short, long, default_value = "todo")]
    /// Label to apply to all created TODOs at the issue provider.
    label: String,

    #[clap(long)]
    /// If supplied, this flag prevents any todos from being created, modified or removed
    /// from the issue provider, and instead the output is printed to stdout as markdown.
    dry_run: bool,

    #[cfg(debug_assertions)]
    #[clap(long)]
    /// Simulate applying the diff patch to the provider during a dry run.
    simulate_application: bool,

    #[clap(short, long)]
    /// An authorization token, like a personal access token.
    auth: String,
}

impl GitHubProvider {
    #[allow(unused_mut)]
    fn should_simulate_application(&self) -> bool {
        let mut should_simulate_application = false;
        #[cfg(debug_assertions)]
        {
            should_simulate_application |= self.simulate_application;
        }
        should_simulate_application
    }
}

#[derive(Debug, Default, Clone, clap::Parser)]
enum IssueProvider {
    /// Use github as the TODO issue provider
    Github(GitHubProvider),
    /// Use a markdown file written to stdout as the TODO issue provider
    #[default]
    Markdown,
}

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
    #[clap(short, long)]
    /// Regular expression of files or directories to ignore,
    /// may be supplied multiple times.
    exclude: Vec<String>,

    #[clap(subcommand)]
    /// The issue provider, eg GitHub or "markdown" for a local file
    provider: IssueProvider,
}

struct Printer {
    red: Style,
    yellow: Style,
    blue: Style,
    green: Style,
    dim: Style,
    is_markdown: bool,
    _multi_progress: MultiProgress,
    found_todos_progress: ProgressBar,
    fetching_issues_progress: ProgressBar,
    patch_create_progress: ProgressBar,
    patch_edit_progress: ProgressBar,
    patch_delete_progress: ProgressBar,
}

impl Default for Printer {
    fn default() -> Self {
        let red = Style::new().red();
        let yellow = Style::new().yellow();
        let green = Style::new().green();
        let blue = Style::new().blue();
        let dim = Style::new().dim();
        let spinner_style = indicatif::ProgressStyle::with_template("{spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        let multi_progress = indicatif::MultiProgress::new();
        let found_todos_progress = multi_progress.add(ProgressBar::new_spinner());
        found_todos_progress.set_style(spinner_style.clone());
        let fetching_issues_progress = multi_progress.add(ProgressBar::new_spinner());
        fetching_issues_progress.set_style(spinner_style.clone());
        let patch_style = ProgressStyle::with_template("{spinner} {prefix} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        let patch_create_progress = multi_progress.add(ProgressBar::new_spinner());
        patch_create_progress.set_style(patch_style.clone());
        let patch_edit_progress = multi_progress.add(ProgressBar::new_spinner());
        patch_edit_progress.set_style(patch_style.clone());
        let patch_delete_progress = multi_progress.add(ProgressBar::new_spinner());
        patch_delete_progress.set_style(patch_style.clone());

        Self {
            red,
            yellow,
            blue,
            green,
            dim,
            is_markdown: true,
            _multi_progress: multi_progress,
            found_todos_progress,
            fetching_issues_progress,
            patch_create_progress,
            patch_edit_progress,
            patch_delete_progress,
        }
    }
}

impl Printer {
    fn print(&mut self, msg: Message) {
        use Message::*;

        match msg {
            Error(err) => {
                let e = self.red.apply_to(err.to_string());
                eprintln!("{e}");
                if let todo_finder_lib::Error::Command { stdout, stderr, .. } = err {
                    eprintln!("  stdout: {stdout}");
                    eprintln!("  stderr: {stderr}");
                }
            }

            GettingOrigin => eprintln!("Getting git origin..."),

            GotOrigin { origin } => eprintln!("  origin '{origin}'"),

            GettingOwnerRepo => eprintln!("Getting owner and repo..."),
            GotOwnerRepo { owner, repo } => eprintln!("  owner '{owner}', repo '{repo}'"),

            GettingCheckoutHash => eprintln!("Getting checkout hash..."),
            GotCheckoutHash { hash } => eprintln!("  checkout hash '{hash}'"),

            FindingTodosInSourceCode => eprintln!("Finding TODOs in source code..."),
            UnsupportedFile { path, todo } => {
                self.found_todos_progress.finish_and_clear();
                eprintln!(
                    "{}",
                    self.yellow
                        .apply_to("  found a possible TODO in an unsupported file:"),
                );
                eprintln!("    {} {}", path.display(), self.dim.apply_to(todo),);
            }
            FoundTodo => {
                self.found_todos_progress.set_message(format!(
                    "Found {} TODOs",
                    self.found_todos_progress.length().unwrap_or_default()
                ));
                self.found_todos_progress.inc(1);
            }
            FoundTodos {
                distinct,
                total,
                markdown_text,
            } => {
                self.found_todos_progress.finish_and_clear();
                eprintln!("Found {distinct} distinct TODOs out of {total} total:\n",);
                if !self.is_markdown {
                    // Only print the markdown when it wouldn't otherwise be printed
                    // or saved to a file
                    let markdown_style = Style::new().dim();
                    println!("{}", markdown_style.apply_to(markdown_text));
                }
            }

            GettingIssues => {
                self.fetching_issues_progress
                    .set_message("Fetching issues from the provider");
            }
            GotIssues { count } => {
                self.fetching_issues_progress.finish_and_clear();
                eprintln!("Got {count} existing TODO issues from the provider");
            }

            PreparedPatch {
                create,
                update,
                delete,
                dry_run,
            } => {
                let create_msg = self.blue.apply_to(format!(
                    "create {create} issue{}",
                    if create == 1 { "" } else { "s" }
                ));
                let update_msg = self.yellow.apply_to(format!(
                    "modify {update} issue{}",
                    if update == 1 { "" } else { "s" }
                ));
                let delete_msg = self.red.apply_to(format!(
                    "delete {delete} issue{}",
                    if delete == 1 { "" } else { "s" }
                ));
                eprintln!("Patching the issue provider would...\n  {create_msg}\n  {update_msg}\n  {delete_msg}");
                if !dry_run {}
            }
            ApplyingPatch {
                create,
                update,
                delete,
            } => {
                eprintln!("Patching issues at the provider...");

                let dur = std::time::Duration::from_millis(1000 / 12);
                if create > 0 {
                    self.patch_create_progress.enable_steady_tick(dur);
                    self.patch_create_progress.set_length(create as u64);
                    self.patch_create_progress.set_length(create as u64);
                    self.patch_create_progress
                        .set_prefix(format!("[0/{create}]"));
                    self.patch_create_progress
                        .set_message(format!("{}", self.blue.apply_to("Creating TODOs")));
                }

                if update > 0 {
                    self.patch_edit_progress.enable_steady_tick(dur);
                    self.patch_edit_progress.set_length(update as u64);
                    self.patch_edit_progress.set_length(update as u64);
                    self.patch_edit_progress.set_prefix(format!("[0/{update}]"));
                    self.patch_edit_progress
                        .set_message(self.yellow.apply_to("Updating TODOs").to_string());
                }

                if delete > 0 {
                    self.patch_delete_progress.set_length(delete as u64);
                    self.patch_delete_progress.enable_steady_tick(dur);
                    self.patch_delete_progress.set_length(delete as u64);
                    self.patch_delete_progress
                        .set_prefix(format!("[0/{delete}]"));
                    self.patch_delete_progress
                        .set_message(self.red.apply_to("Deleting TODOs").to_string());
                }
            }
            AppliedPatchCreate { done, total } => {
                self.print_applied_patch(done, total, &self.patch_create_progress);
            }
            AppliedPatchUpdate { done, total } => {
                self.print_applied_patch(done, total, &self.patch_edit_progress);
            }
            AppliedPatchDelete { done, total } => {
                self.print_applied_patch(done, total, &self.patch_delete_progress);
            }
            AppliedPatch => {
                self.patch_create_progress.finish_and_clear();
                self.patch_edit_progress.finish_and_clear();
                self.patch_delete_progress.finish_and_clear();
            }

            Goodbye => {
                eprintln!("🏁 {}", self.green.apply_to("All done!"));
            }
        }
    }

    fn print_applied_patch(&self, done: usize, total: usize, progress: &ProgressBar) {
        progress.set_position(done as u64);
        if done == total {
            progress.set_prefix("complete");
        } else {
            progress.set_prefix(format!("[{done}/{total}]"));
        }
    }

    async fn message_loop(&mut self, handle: tokio::task::JoinHandle<()>) {
        let recv = todo_finder_lib::Message::receiver();
        loop {
            let mut timeout =
                std::pin::pin!(tokio::time::sleep(std::time::Duration::from_secs(1)).fuse());
            let mut get_msg = std::pin::pin!(recv.recv().fuse());
            futures::select! {
                msg = get_msg => if let Ok(msg ) = msg {
                    self.print(msg);
                },
                _ = timeout => {}
            }
            if handle.is_finished() {
                break;
            }
        }
        while let Ok(msg) = recv.try_recv() {
            self.print(msg);
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::builder().init();

    let cwd = std::env::current_dir().expect("could not get current dir");
    let cwd_str = cwd.to_str().expect("could not convert cwd path").to_owned();
    let cli = Cli::parse();
    let Cli { exclude, provider } = cli;

    eprintln!("🌈 Starting todo_finder...");

    let mut printer = Printer::default();
    let handle = match provider {
        IssueProvider::Markdown => {
            printer.is_markdown = true;
            tokio::task::spawn(async move {
                let issues = IssueMap::from_files_in_directory(&cwd_str, &exclude)
                    .await
                    .unwrap();
                let markdown = issues.as_markdown();
                println!("{markdown}")
            })
        }

        IssueProvider::Github(gh) => {
            printer.is_markdown = false;
            let simulate_application = gh.should_simulate_application();
            let finder = github::run(
                gh.auth,
                gh.label,
                cwd_str,
                exclude,
                gh.dry_run,
                simulate_application,
            );
            // let term = console::Term::stdout();
            tokio::task::spawn(finder)
        }
    };

    // While the finder is working, print the messages to the terminal
    printer.message_loop(handle).await;
}
