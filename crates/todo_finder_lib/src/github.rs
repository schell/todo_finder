use std::{future::Future, pin::Pin};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::ParseOwnerRepoSnafu;

use super::{
    finder::parse::parse_owner_and_repo_from_config,
    parser::{issue::*, FileTodoLocation, IssueMap},
    Message, Result,
};

pub struct GitHubPatch {
    pub create: IssueMap<(), FileTodoLocation>,
    pub edit: IssueMap<u64, FileTodoLocation>,
    pub delete: Vec<u64>,
}

pub async fn run(
    auth_token: String,
    issue_label: String,
    cwd: String,
    excludes: Vec<String>,
    dry_run: bool,
    simulate_application: bool,
) {
    let mut finder = match Finder::new(
        auth_token,
        issue_label,
        cwd,
        excludes,
        dry_run,
        simulate_application,
    ) {
        Ok(finder) => finder,
        Err(e) => return Message::Error(e).send(),
    };
    match finder.run().await {
        Ok(()) => Message::Goodbye.send(),
        Err(e) => Message::Error(e).send(),
    }
}

struct Finder {
    api: octocrab::Octocrab,
    cwd: String,
    issue_label: String,
    excludes: Vec<String>,
    dry_run: bool,
    simulate_application: bool,
}

impl Finder {
    pub fn new(
        auth_token: String,
        issue_label: String,
        cwd: String,
        excludes: Vec<String>,
        dry_run: bool,
        simulate_application: bool,
    ) -> Result<Self> {
        let api = octocrab::Octocrab::builder()
            .user_access_token(auth_token.clone())
            .build()?;

        Ok(Self {
            api,
            cwd,
            issue_label,
            excludes,
            dry_run,
            simulate_application,
        })
    }

    async fn get_github_issues(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<IssueMap<u64, GitHubTodoLocation>> {
        Message::GettingIssues.send();

        let mut issues = IssueMap::new_github_todos();
        let page_of_issues = self
            .api
            .issues(owner, repo)
            .list()
            .labels(std::slice::from_ref(&self.issue_label))
            .send()
            .await?;
        let mut all_issues_stream = std::pin::pin!(page_of_issues.into_stream(&self.api));
        while let Some(result) = all_issues_stream.next().await {
            let issue = result?;
            issues.add_issue(&issue);
        }

        Message::GotIssues {
            count: issues.todos.len(),
        }
        .send();

        Ok(issues)
    }

    async fn apply_patch(
        &self,
        owner: &str,
        repo: &str,
        checkout_hash: &str,
        GitHubPatch {
            create,
            edit,
            delete,
        }: GitHubPatch,
    ) -> Result<()> {
        let create_total = create.distinct_len();
        let delete_total = delete.len();
        let edit_total = edit.todos.len();
        let root_project_dir = &self.cwd;

        Message::ApplyingPatch {
            create: create_total,
            update: edit_total,
            delete: delete_total,
        }
        .send();

        let mut issues: Vec<Pin<Box<dyn Future<Output = Result<()>> + Send>>> = vec![];
        // Create
        for (i, (_, issue)) in create.todos.into_iter().enumerate() {
            issues.push(Box::pin(async move {
                self.api
                    .issues(owner, repo)
                    .create(&issue.head.title)
                    .body(issue.body.to_github_string(
                        root_project_dir,
                        owner,
                        repo,
                        checkout_hash,
                    )?)
                    .assignees(Some(issue.head.assignees.clone()))
                    .labels(Some(vec![self.issue_label.clone()]))
                    .send()
                    .await?;
                Message::AppliedPatchCreate {
                    done: i,
                    total: create_total,
                }
                .send();
                Ok(())
            }));
        }

        // Edit
        for (i, (_, issue)) in edit.todos.into_iter().enumerate() {
            let id = issue.head.external_id;
            let body = issue
                .body
                .to_github_string(root_project_dir, owner, repo, checkout_hash)?;
            issues.push(Box::pin(async move {
                let gh_issue = self.api.issues(owner, repo).get(id).await?;
                let mut labels = gh_issue
                    .labels
                    .iter()
                    .map(|label| label.name.clone())
                    .collect::<Vec<_>>();
                if !labels.contains(&self.issue_label) {
                    labels.push(self.issue_label.clone());
                }

                let _res_issue = self
                    .api
                    .issues(owner, repo)
                    .update(id)
                    .title(&issue.head.title)
                    .body(&body)
                    .assignees(&issue.head.assignees)
                    .labels(&labels)
                    .send()
                    .await?;
                Message::AppliedPatchUpdate {
                    done: i,
                    total: edit_total,
                }
                .send();
                Ok(())
            }));
        }

        // Delete
        for (done, id) in delete.into_iter().enumerate() {
            issues.push(Box::pin(async move {
                self.api
                    .issues(owner, repo)
                    .update(id)
                    .state(octocrab::models::IssueState::Closed)
                    .send()
                    .await?;
                Message::AppliedPatchDelete {
                    done,
                    total: delete_total,
                }
                .send();
                Ok(())
            }));
        }

        let mut issue_stream = futures::stream::iter(issues).buffer_unordered(3);
        while issue_stream.next().await.is_some() {}

        Message::AppliedPatch.send();
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        log::debug!("starting the find");
        let origin = crate::utils::git_origin().await?;

        Message::GettingOwnerRepo.send();
        let (owner, repo) = parse_owner_and_repo_from_config(&origin)
            .map_err(|_| ParseOwnerRepoSnafu.build())?
            .1;
        Message::GotOwnerRepo {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        }
        .send();

        let checkout_hash = crate::utils::git_hash().await?;

        let local_issues = IssueMap::from_files_in_directory(&self.cwd, &self.excludes).await?;

        let remote_issues = self.get_github_issues(owner, repo).await?;
        let patch = remote_issues.prepare_patch(local_issues);
        let create = patch.create.distinct_len();
        let update = patch.edit.distinct_len();
        let delete = patch.delete.len();
        Message::PreparedPatch {
            create,
            update,
            delete,
            dry_run: self.dry_run,
        }
        .send();

        log::debug!(
            "dry_run: {}, simulating: {}",
            self.dry_run,
            self.simulate_application
        );
        if self.dry_run && self.simulate_application {
            log::debug!("simulating apply");
            Message::ApplyingPatch {
                create,
                update,
                delete,
            }
            .send();

            let mut rando_awaits: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send>>> =
                FuturesUnordered::default();
            for n in 1..=create {
                rando_awaits.push(Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(n as u64)).await;
                    Message::AppliedPatchCreate {
                        done: n,
                        total: create,
                    }
                    .send();
                }));
            }
            for n in 1..=update {
                rando_awaits.push(Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(n as u64)).await;
                    Message::AppliedPatchUpdate {
                        done: n,
                        total: update,
                    }
                    .send();
                }));
            }
            for n in 1..=delete {
                rando_awaits.push(Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(n as u64)).await;
                    Message::AppliedPatchDelete {
                        done: n,
                        total: delete,
                    }
                    .send();
                }));
            }

            while rando_awaits.next().await.is_some() {}
            Message::AppliedPatch.send();
        } else if !self.dry_run {
            self.apply_patch(owner, repo, &checkout_hash, patch).await?;
        }

        Ok(())
    }
}
