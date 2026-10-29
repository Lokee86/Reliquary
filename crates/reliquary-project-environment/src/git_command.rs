use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_output(root, args, None, &[])?;
    String::from_utf8(output)
        .map(|value| value.trim().to_string())
        .map_err(|_| "Git returned non-UTF-8 text".to_string())
}

pub(crate) fn git_bytes_with_input(
    root: &Path,
    args: &[&str],
    input: &[u8],
) -> Result<Vec<u8>, String> {
    git_output(root, args, Some(input), &[])
}

pub(crate) fn git_with_index(
    root: &Path,
    args: &[OsString],
    index: &Path,
    input: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let env = [(
        OsString::from("GIT_INDEX_FILE"),
        index.as_os_str().to_owned(),
    )];
    git_output_os(root, args, input, &env)
}

pub(crate) fn git_owned(root: &Path, args: &[OsString]) -> Result<Vec<u8>, String> {
    git_output_os(root, args, None, &[])
}

pub(crate) fn git_success(root: &Path, args: &[&str]) -> Result<(), String> {
    git_output(root, args, None, &[]).map(|_| ())
}

fn git_output(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    env: &[(OsString, OsString)],
) -> Result<Vec<u8>, String> {
    let owned = args.iter().map(OsString::from).collect::<Vec<_>>();
    git_output_os(root, &owned, input, env)
}

fn git_output_os(
    root: &Path,
    args: &[OsString],
    input: Option<&[u8]>,
    env: &[(OsString, OsString)],
) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .envs(env.iter().cloned());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to execute Git: {error}"))?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .ok_or_else(|| "Git stdin is unavailable".to_string())?
            .write_all(input)
            .map_err(|error| format!("failed to write Git stdin: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for Git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_failure("Git command", &output.stderr))
    }
}

pub(crate) fn git_failure(operation: &str, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    if detail.is_empty() {
        format!("{operation} failed")
    } else {
        format!("{operation} failed: {detail}")
    }
}
