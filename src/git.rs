use anyhow::{Result, anyhow};
use std::process::{Command, Output};
use tokio::task;

/// 异步执行 git add / commit / push
pub async fn git_commit_and_push(path: &'static str) -> Result<()> {
    task::spawn_blocking(move || git_commit_and_push_blocking(path))
        .await
        .map_err(|e| anyhow!("git 任务执行失败: {e}"))?
}

/// 同步执行 git add / commit / push
fn git_commit_and_push_blocking(path: &str) -> Result<()> {
    run_git(&["add", path])?;

    if !has_staged_changes(path)? {
        log::info!("{path} 没有变化, 跳过 git commit 和 git push");
        return Ok(());
    }

    let commit_message = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    run_git(&["commit", "-m", &commit_message])?;
    run_git(&["push", "origin", "HEAD"])?;

    log::info!("{path} 已提交并推送到远程仓库");
    Ok(())
}

/// 检查 staged 区中该文件是否存在变化
fn has_staged_changes(path: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet", "--", path])
        .output()?;

    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        Some(code) => Err(anyhow!(
            "git diff --cached --quiet 执行失败, 退出码: {code}, stderr: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        None => Err(anyhow!("git diff --cached --quiet 被信号中断")),
    }
}

/// 执行 git 命令
fn run_git(args: &[&str]) -> Result<Output> {
    let output = Command::new("git").args(args).output()?;

    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Err(anyhow!(
        "git {:?} 执行失败, stdout: {}, stderr: {}",
        args,
        stdout,
        stderr
    ))
}
