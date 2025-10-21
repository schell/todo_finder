//! Command utilities and stuff.

use std::process::Stdio;

use crate::{CommandSnafu, Error, Message};

pub async fn command(
    command: &mut tokio::process::Command,
    cmd: &'static str,
) -> Result<String, Error> {
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let output = child.wait_with_output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    snafu::ensure!(
        output.status.success(),
        CommandSnafu {
            status: output.status,
            cmd,
            stdout,
            stderr: String::from_utf8_lossy(&output.stderr).clone(),
        }
    );
    Ok(stdout)
}

/// git config --get remote.origin.url
pub async fn git_origin() -> Result<String, Error> {
    Message::GettingOrigin.send();
    let s = command(
        tokio::process::Command::new("git").args(["config", "--get", "remote.origin.url"]),
        "git config --get remote.origin.url",
    )
    .await?;
    Message::GotOrigin { origin: s.clone() }.send();
    Ok(s)
}

/// git rev-parse HEAD
pub async fn git_hash() -> Result<String, Error> {
    Message::GettingCheckoutHash.send();
    let s = command(
        tokio::process::Command::new("git")
            .arg("rev-parse")
            .arg("HEAD"),
        "git rev-parse HEAD",
    )
    .await?;
    Message::GotCheckoutHash { hash: s.clone() }.send();
    Ok(s)
}

/// Run `rg` with the path and pattern given, returning the result bytes if
/// successful.
pub async fn get_rg_output(
    path: &str,
    pattern: &str,
    excludes: &[String],
) -> Result<Vec<u8>, Error> {
    let mut args = vec![];
    args.extend(["--heading", "--line-number"].map(|s| s.to_owned()));
    for exclude in excludes.iter() {
        args.extend(["-g".to_owned(), format!("!{}", exclude)]);
    }
    args.push(pattern.to_owned());
    args.push(path.to_owned());

    log::trace!("running rg:\nrg {}", args.clone().join(" "));
    let child = tokio::process::Command::new("rg")
        .args(args.clone())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let output = child.wait_with_output().await?;
    let stdout = output.stdout;
    if output.status.success() {
        Ok(stdout)
    } else {
        // For some reason rg returns an error when there are no results...
        Ok(vec![])
    }
}
