use std::{
    path::Path,
    process::{Command, Output},
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Project {
    pub folder_name: String,
    pub branch: String,
    pub repository: Option<String>,
}

// Never inherit another checkout's Git context from the MCP host.
fn git(directory: &Path, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("LC_ALL", "C")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .map_err(|_| {
            "Could not run git; install Git and ensure it is on the MCP server's PATH".into()
        })
}

fn output_text(output: Output) -> Result<String, String> {
    if !output.status.success() {
        return Err("Git metadata discovery failed; check directory access and Git repository configuration".into());
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim_end_matches(['\n', '\r']).to_owned())
        .map_err(|_| "Git metadata must be valid UTF-8".into())
}

pub(super) fn resolve(working_directory: &str) -> Result<Project, String> {
    let directory = Path::new(working_directory);
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("working_directory must be an existing absolute directory".into());
    }
    let directory = directory
        .canonicalize()
        .map_err(|_| "Cannot resolve working_directory")?;
    let root = git(&directory, &["rev-parse", "--show-toplevel"])?;
    let (root, branch, repository) = if root.status.success() {
        let root = output_text(root)?;
        let branch = git(&directory, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let branch = if branch.status.code() == Some(1) {
            String::new()
        } else {
            output_text(branch)?
        };
        let remote = git(&directory, &["remote", "get-url", "origin"])?;
        let repository = if remote.status.code() == Some(2) {
            None // No origin configured.
        } else {
            normalize_remote(&output_text(remote)?)
        };
        (std::path::PathBuf::from(root), branch, repository)
    } else if String::from_utf8_lossy(&root.stderr).contains("not a git repository") {
        (directory, String::new(), None)
    } else {
        return Err(
            "Cannot discover Git checkout; check repository access and configuration".into(),
        );
    };
    let folder_name = root
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or("Project directory must have a UTF-8 folder name")?
        .to_owned();
    Ok(Project {
        folder_name,
        branch,
        repository,
    })
}

fn normalize_remote(remote: &str) -> Option<String> {
    let path = if let Some((scheme, rest)) = remote.split_once("://") {
        if !matches!(scheme, "https" | "http" | "ssh" | "git") {
            return None;
        }
        rest.split_once('/')?.1
    } else {
        // SCP-style remotes, e.g. git@github.com:owner/repo.git.
        let (host, path) = remote.split_once(':')?;
        if host.contains('/') || host.is_empty() {
            return None;
        }
        path
    };
    let path = path
        .trim_end_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_end_matches('/'));
    let segments: Vec<_> = path.split('/').collect();
    if segments.len() < 2
        || segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
        || path.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(directory: &Path, args: &[&str]) {
        assert!(
            git(directory, args).unwrap().status.success(),
            "git {args:?}"
        );
    }

    #[test]
    fn normalizes_network_remotes_without_inventing_local_repository_names() {
        for remote in [
            "git@github.com:Vivek-Raman/Sideglance.git",
            "https://github.com/Vivek-Raman/Sideglance.git/",
            "ssh://git@github.com:2222/Vivek-Raman/Sideglance.git",
            "git://github.com/Vivek-Raman/Sideglance",
        ] {
            assert_eq!(
                normalize_remote(remote).as_deref(),
                Some("Vivek-Raman/Sideglance")
            );
        }
        assert_eq!(
            normalize_remote("https://gitlab.com/group/sub/repo.git").as_deref(),
            Some("group/sub/repo")
        );
        for remote in [
            "/tmp/owner/repo",
            "../repo",
            "file:///tmp/owner/repo",
            "https://host/repo",
            "https://host/owner//repo",
        ] {
            assert_eq!(normalize_remote(remote), None);
        }
    }

    #[test]
    fn resolves_subdirectories_branch_changes_detached_head_and_worktrees() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("checkout");
        std::fs::create_dir(&root).unwrap();
        run(&root, &["init", "-b", "main"]);
        run(
            &root,
            &["remote", "add", "origin", "git@github.com:owner/repo.git"],
        );
        let subdir = root.join("nested");
        std::fs::create_dir(&subdir).unwrap();
        let resolve_at = |p: &Path| resolve(p.to_str().unwrap()).unwrap();
        assert_eq!(
            resolve_at(&subdir),
            Project {
                folder_name: "checkout".into(),
                branch: "main".into(),
                repository: Some("owner/repo".into())
            }
        );
        run(
            &root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "init",
            ],
        );
        run(&root, &["checkout", "-b", "feature"]);
        assert_eq!(resolve_at(&root).branch, "feature");
        run(&root, &["checkout", "--detach"]);
        assert_eq!(resolve_at(&root).branch, "");
        let worktree = temp.path().join("worktree");
        run(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                "parallel",
                worktree.to_str().unwrap(),
            ],
        );
        assert_eq!(
            resolve_at(&worktree),
            Project {
                folder_name: "worktree".into(),
                branch: "parallel".into(),
                repository: Some("owner/repo".into())
            }
        );
        run(&root, &["remote", "remove", "origin"]);
        assert_eq!(resolve_at(&root).repository, None);
    }

    #[test]
    fn validates_paths_and_supports_non_git_directories() {
        assert!(resolve("relative").is_err());
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("outside");
        std::fs::create_dir(&directory).unwrap();
        assert!(resolve(directory.join("missing").to_str().unwrap()).is_err());
        // Temp directories must be outside a Git checkout for this fallback.
        let project = resolve(directory.to_str().unwrap()).unwrap();
        assert_eq!(project.folder_name, "outside");
        assert_eq!(project.branch, "");
        assert_eq!(project.repository, None);
    }
}
