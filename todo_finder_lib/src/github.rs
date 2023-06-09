use super::{
    finder::parse::parse_owner_and_repo_from_config,
    parser::{issue::*, FileTodoLocation, IssueMap},
};
use hyper::{
    body::{Body, HttpBody},
    Client, Request, Response,
};
use hyper_tls::HttpsConnector;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;


#[derive(Deserialize)]
struct GitHubConfig {
    // Label to use for filtering TODO issues
    issue_label: String,
    // Github token
    auth_token: String,
    // Where do we search for TODOs
    _search_in_directory: Option<String>,
    // The repo owner
    owner: String,
    // The repo name
    repo: String,
    // The current checkout hash
    checkout_hash: String,
    // The root project directory
    root_project_dir: String,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubLabel {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubAssignee {
    pub login: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub labels: Vec<GitHubLabel>,
    pub assignees: Vec<GitHubAssignee>,
    pub user: GitHubUser,
}


pub struct GitHubPatch {
    pub create: IssueMap<(), FileTodoLocation>,
    pub edit: IssueMap<u64, FileTodoLocation>,
    pub delete: Vec<u64>,
}


pub fn github_issues_url(owner: &str, repo: &str) -> String {
    format!("https://api.github.com/repos/{}/{}/issues", owner, repo)
}


pub fn github_issues_update_url(owner: &str, repo: &str, id: u64) -> String {
    format!(
        "https://api.github.com/repos/{}/{}/issues/{}",
        owner, repo, id
    )
}


/// git config --get remote.origin.url
pub fn git_origin() -> Result<String, String> {
    let output = Command::new("git")
        .arg("config")
        .arg("--get")
        .arg("remote.origin.url")
        .output()
        .map_err(|e| format!("could not determine the git origin: {}", e))?;

    if !output.status.success() {
        let output = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("git config --get remote.origin.url: '{}'", output));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}


/// git rev-parse HEAD
pub fn git_hash() -> Result<String, String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|e| format!("could not run git rev-parse HEAD: {}", e))?;

    if !output.status.success() {
        return Err("git rev-parse HEAD erred".into());
    }

    let s: String = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(s)
}


async fn get_github_issues(
    cfg: &GitHubConfig,
) -> Result<IssueMap<u64, GitHubTodoLocation>, String> {
    let url = github_issues_url(&cfg.owner, &cfg.repo);
    println!("  {}", url);
    let req = github_req(
        cfg,
        "GET",
        &url,
        json!({
          "labels": vec![&cfg.issue_label],
          "state": "open"
        }),
    )?;

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    let res = client
        .request(req)
        .await
        .map_err(|e| format!("error fetching github issues: {}", e))?;
    let github_issues: Vec<GitHubIssue> = get_json_response(res).await?;
    let mut issues = IssueMap::new_github_todos();
    for issue in github_issues.iter() {
        issues.add_issue(issue);
    }

    Ok(issues)
}


fn github_req<T: Serialize>(
    cfg: &GitHubConfig,
    method: &str,
    uri: &str,
    body: T,
) -> Result<Request<Body>, String> {
    let json_data = serde_json::to_string(&body)
        .map_err(|e| format!("could not serialize request body: {}", e))?;
    Request::builder()
        .method(method)
        .uri(uri)
        .header("User-Agent", &cfg.owner)
        .header("Accept", "application/json")
        .header("Authorization", format!("token {}", &cfg.auth_token))
        .body(json_data.into())
        .map_err(|e| format!("error building github request: {} {}", uri, e))
}


async fn get_json_response<T: DeserializeOwned>(mut res: Response<Body>) -> Result<T, String> {
    //println!("Response: {}", res.status());
    //println!("Headers: {:#?}\n", res.headers());

    // Stream the body, buffering each chunk to a string as we get it
    let mut chunks: Vec<String> = vec![];
    while let Some(next) = res.data().await {
        let chunk = next.map_err(|e| format!("error getting next chunk: {}", e))?;
        let chunk = String::from_utf8_lossy(&chunk).to_string();
        chunks.push(chunk);
    }
    let json_string = chunks.concat();
    serde_json::from_str::<T>(&json_string).map_err(|e| {
        format!(
            "could not deserialize github response: {}\nbody: {}",
            e, json_string
        )
    })
}


async fn apply_patch(cfg: &GitHubConfig, patch: GitHubPatch) -> Result<(), String> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let url = github_issues_url(&cfg.owner, &cfg.repo);

    // Create
    println!("creating {} issues", patch.create.todos.len());
    for (_, issue) in patch.create.todos.iter() {
        let req = github_req(
            &cfg,
            "POST",
            &url,
            json!({
              "title": issue.head.title,
              "body": issue.body.to_github_string(
                &cfg.root_project_dir,
                &cfg.owner,
                &cfg.repo,
                &cfg.checkout_hash
              )?,
              "assignees": issue.head.assignees,
              "labels": vec![&cfg.issue_label]
            }),
        )?;
        let res: Response<Body> = client
            .request(req)
            .await
            .map_err(|e| format!("error creating github issue: {}", e))?;

        let _val: Value = get_json_response(res).await?;
        println!("created '{}':", issue.head.title);
        //println!("{:#?}", val);
    }

    // Edit
    println!("editing {} issues", patch.edit.todos.len());
    for (_, issue) in patch.edit.todos.iter() {
        println!("editing '{}'", issue.head.title);
        let id = issue.head.external_id;
        let body = issue
            .body
            .to_github_string(
                &cfg.root_project_dir,
                &cfg.owner,
                &cfg.repo,
                &cfg.checkout_hash,
            )
            .map_err(|e| format!("could not convert issue body to description: {}", e))?;
        let print_body = body
            .lines()
            .map(|s| vec!["  ".into(), s].concat())
            .collect::<Vec<_>>()
            .join("\n");
        println!("{}", print_body);

        let req = github_req(
            &cfg,
            "PATCH",
            &github_issues_update_url(&cfg.owner, &cfg.repo, id),
            json!({
              "title": issue.head.title,
              "body": body,
              "assignees": issue.head.assignees,
              "labels": vec![&cfg.issue_label]
            }),
        )?;
        let res: Response<Body> = client
            .request(req)
            .await
            .map_err(|e| format!("error editing github issue: {}", e))?;

        let _: Value = get_json_response(res).await?;
    }

    // Delete
    println!("deleting {} issues", patch.delete.len());
    for id in patch.delete.iter() {
        let req = github_req(
            &cfg,
            "PATCH",
            &github_issues_update_url(&cfg.owner, &cfg.repo, *id),
            json!({"state":"closed"}),
        )?;
        let res = client
            .request(req)
            .await
            .map_err(|e| format!("error closing github issue: {}", e))?;

        let json: Value = get_json_response(res).await?;
        let title = json
            .as_object()
            .map(|obj| obj.get("title").map(|s| s.as_str()).flatten())
            .flatten();
        if let Some(title) = title {
            println!("closed '{}'", title);
        }
    }

    Ok(())
}


pub async fn run_ts_github(
    auth_token: String,
    issue_label: String,
    cwd: String,
    excludes: &Vec<String>,
) -> Result<(), String> {
    //let path = Path::new(config_path_str);
    //let mut file: File = File::open(path).expect("could not open config file");
    //let mut contents = String::new();
    //file
    //  .read_to_string(&mut contents)
    //  .map_err(|e| format!("could not read config file {:#?}", e))?;

    //let config: ConfigFile = serde_yaml::from_str(&contents)
    //  .map_err(|e| format!("could not read config: {}", e))?;

    let origin = git_origin()?;
    println!("origin: {}", origin);
    let (owner, repo) = parse_owner_and_repo_from_config(&origin)
        .map_err(|_| "could not parse owner/repo from git config".to_string())?
        .1;
    println!("owner: '{}', repo: '{}'", owner, repo);
    let checkout_hash = git_hash()?;
    let local_issues = IssueMap::from_files_in_directory(&cwd, excludes).unwrap();
    let num_issues = local_issues.distinct_len();
    if num_issues > 0 {
        println!("Found {} distinct local TODOs", num_issues);
    }

    // Find the issues at the issue provider
    let cfg = GitHubConfig {
        issue_label,
        auth_token,
        _search_in_directory: None,
        owner: owner.into(),
        repo: repo.into(),
        checkout_hash,
        root_project_dir: cwd,
    };

    println!("Getting remote issues for {}/{}", owner, repo);
    let remote_issues = get_github_issues(&cfg).await?;

    let patch = remote_issues.prepare_patch(local_issues);

    println!("Patching remote issues");
    apply_patch(&cfg, patch).await?;

    Ok(())
}

#[cfg(test)]
mod regression {
    use super::*;

    const GITHUB_ISSUE_TEXT: &str = r#"[
  {
    "url": "https://api.github.com/repos/schell/renderling/issues/10",
    "repository_url": "https://api.github.com/repos/schell/renderling",
    "labels_url": "https://api.github.com/repos/schell/renderling/issues/10/labels{/name}",
    "comments_url": "https://api.github.com/repos/schell/renderling/issues/10/comments",
    "events_url": "https://api.github.com/repos/schell/renderling/issues/10/events",
    "html_url": "https://github.com/schell/renderling/issues/10",
    "id": 1700888990,
    "node_id": "I_kwDOG2DFqM5lYYGe",
    "number": 10,
    "title": "remove this as the `atlas` field is public now",
    "user": {
      "login": "schell",
      "id": 24942,
      "node_id": "MDQ6VXNlcjI0OTQy",
      "avatar_url": "https://avatars.githubusercontent.com/u/24942?v=4",
      "gravatar_id": "",
      "url": "https://api.github.com/users/schell",
      "html_url": "https://github.com/schell",
      "followers_url": "https://api.github.com/users/schell/followers",
      "following_url": "https://api.github.com/users/schell/following{/other_user}",
      "gists_url": "https://api.github.com/users/schell/gists{/gist_id}",
      "starred_url": "https://api.github.com/users/schell/starred{/owner}{/repo}",
      "subscriptions_url": "https://api.github.com/users/schell/subscriptions",
      "organizations_url": "https://api.github.com/users/schell/orgs",
      "repos_url": "https://api.github.com/users/schell/repos",
      "events_url": "https://api.github.com/users/schell/events{/privacy}",
      "received_events_url": "https://api.github.com/users/schell/received_events",
      "type": "User",
      "site_admin": false
    },
    "labels": [
      {
        "id": 5480519678,
        "node_id": "LA_kwDOG2DFqM8AAAABRqoX_g",
        "url": "https://api.github.com/repos/schell/renderling/labels/todo",
        "name": "todo",
        "color": "f9d0c4",
        "default": false,
        "description": "TODO: ..."
      }
    ],
    "state": "open",
    "locked": false,
    "assignee": null,
    "assignees": [],
    "milestone": null,
    "comments": 0,
    "created_at": "2023-05-08T20:28:26Z",
    "updated_at": "2023-05-08T20:28:27Z",
    "closed_at": null,
    "author_association": "OWNER",
    "active_lock_reason": null,
    "body": "\\nhttps://github.com/schell/renderling/blob/9e5451d6fa5ce074af4df752063d8b6b1a9c938b/crates/renderling/src/scene.rs#L482",
    "reactions": {
      "url": "https://api.github.com/repos/schell/renderling/issues/10/reactions",
      "total_count": 0,
      "+1": 0,
      "-1": 0,
      "laugh": 0,
      "hooray": 0,
      "confused": 0,
      "heart": 0,
      "rocket": 0,
      "eyes": 0
    },
    "timeline_url": "https://api.github.com/repos/schell/renderling/issues/10/timeline",
    "performed_via_github_app": null,
    "state_reason": null
  }
]"#;

    #[test]
    fn can_deserialize_github_issues() {
        serde_json::from_str::<Vec<GitHubIssue>>(GITHUB_ISSUE_TEXT).unwrap();
    }
}
