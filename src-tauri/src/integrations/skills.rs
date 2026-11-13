use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::{
    integrations::registry::{additional_codex_directory, skill_path, AGENTS},
    paths::home_dir,
    SKILL_NAME,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillInstallation {
    pub installed: bool,
    pub mcp_registered: bool,
    pub path: String,
    pub targets: Vec<SkillTarget>,
    pub additional_targets: Vec<SkillTarget>,
    pub agents: Vec<AgentInstallation>,
    pub additional_agents: Vec<AgentInstallation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillTarget {
    pub id: String,
    pub name: String,
    pub path: String,
    pub installed: bool,
    pub up_to_date: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInstallation {
    pub id: String,
    pub name: String,
    pub skill_installed: bool,
    pub mcp_registered: bool,
    pub configuration_valid: bool,
    pub available: bool,
    pub config_path: String,
    pub error: Option<String>,
}

pub(crate) fn install_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home_dir(app)?
        .join(".agents")
        .join("skills")
        .join(SKILL_NAME))
}

pub(crate) fn bundled_source(app: &AppHandle) -> Result<PathBuf, String> {
    let source = app
        .path()
        .resolve(format!("skills/{SKILL_NAME}"), BaseDirectory::Resource)
        .map_err(|e| format!("Unable to find the bundled skill: {e}"))?;
    if !source.join("SKILL.md").is_file() {
        return Err("The bundled skill is missing SKILL.md".into());
    }
    Ok(source)
}

pub(crate) fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|e| format!("Unable to create {}: {e}", destination.display()))?;
    for entry in
        fs::read_dir(source).map_err(|e| format!("Unable to read {}: {e}", source.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if file_type.is_symlink() {
            return Err(format!(
                "Skill source contains a symbolic link: {}",
                source_path.display()
            ));
        }
        if file_type.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|e| format!("Unable to copy {}: {e}", source_path.display()))?;
        } else {
            return Err(format!("Unsupported skill file: {}", source_path.display()));
        }
    }
    Ok(())
}

pub(crate) fn skill_hash(path: &Path) -> Result<String, String> {
    fn hash_directory(root: &Path, directory: &Path, hasher: &mut Sha256) -> Result<(), String> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| format!("Unable to read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Unable to read {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("Unable to hash {}: {error}", path.display()))?;
            let relative = relative.to_string_lossy();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("Unable to inspect {}: {error}", path.display()))?;
            if file_type.is_dir() {
                hasher.update(b"directory\0");
                hasher.update(relative.as_bytes());
                hasher.update(b"\0");
                hash_directory(root, &path, hasher)?;
            } else if file_type.is_file() {
                hasher.update(b"file\0");
                hasher.update(relative.as_bytes());
                hasher.update(b"\0");
                hasher.update(
                    fs::read(&path)
                        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?,
                );
            }
        }
        Ok(())
    }
    let mut hasher = Sha256::new();
    hash_directory(path, path, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn install_at(source: &Path, destination: &Path) -> Result<bool, String> {
    install_with_rename(source, destination, |from, to| fs::rename(from, to))
}

fn install_with_rename(
    source: &Path,
    destination: &Path,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<bool, String> {
    use fs2::FileExt;
    let source_type = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if !source_type.is_dir() || !source.join("SKILL.md").is_file() {
        return Err("The skill source must be a directory containing SKILL.md".into());
    }
    let parent = destination
        .parent()
        .ok_or("Skill destination has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let name = destination
        .file_name()
        .ok_or("Skill destination has no name")?
        .to_string_lossy();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join(format!(".{name}.install.lock")))
        .map_err(|error| error.to_string())?;
    lock.try_lock_exclusive()
        .map_err(|_| "Another skill installation is in progress; retry shortly".to_string())?;
    let existing = match fs::symlink_metadata(destination) {
        Ok(metadata) => {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || !destination.join("SKILL.md").is_file()
            {
                return Err(format!("{} already exists and is not a regular skill directory. Remove or rename it before installing.", destination.display()));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.to_string()),
    };
    let stage = tempfile::Builder::new()
        .prefix(&format!(".{name}.staging-"))
        .tempdir_in(parent)
        .map_err(|error| error.to_string())?;
    copy_directory(source, stage.path())?;
    if skill_hash(source)? != skill_hash(stage.path())? {
        return Err("Skill source changed during installation; retry the operation".into());
    }
    let backup = parent.join(format!(".{name}.backup-{}", uuid::Uuid::new_v4()));
    if existing {
        rename(destination, &backup)
            .map_err(|error| format!("Unable to back up {}: {error}", destination.display()))?;
    }
    if let Err(error) = rename(stage.path(), destination) {
        if existing {
            rename(&backup, destination).map_err(|restore_error| format!(
                "Installation failed: {error}. Unable to restore previous skill: {restore_error}. Recover it from {}", backup.display()))?;
        }
        return Err(format!(
            "Unable to install {}: {error}. Previous installation preserved.",
            destination.display()
        ));
    }
    // Backups are deliberately retained. A crash between the two renames leaves
    // the old skill in the named backup, never deleted. Recovery is a rename.
    retire_legacy_skill(destination)?;
    Ok(true)
}

fn retire_legacy_skill(destination: &Path) -> Result<(), String> {
    if destination.file_name().and_then(|name| name.to_str()) != Some(SKILL_NAME) {
        return Ok(());
    }
    let legacy = destination.with_file_name("ctx-ppteer").join("SKILL.md");
    if legacy.is_file() {
        // Preserve customized instructions, but remove the discoverable skill entry point.
        let backup = legacy.with_file_name(format!("SKILL.md.{}.backup", uuid::Uuid::new_v4()));
        fs::rename(&legacy, &backup).map_err(|error| {
            format!(
                "Sideglance installed, but unable to retire {}: {error}",
                legacy.display()
            )
        })?;
    }
    Ok(())
}

pub(crate) fn targets(
    app: &AppHandle,
) -> Result<Vec<(&'static str, &'static str, PathBuf)>, String> {
    let home = home_dir(app)?;
    Ok(std::iter::once(("agents", "Agents", install_path(app)?))
        .chain(
            AGENTS
                .iter()
                .filter(|agent| agent.supports_skill)
                .map(|agent| (agent.id, agent.name, skill_path(&home, agent))),
        )
        .collect())
}

pub(crate) fn target(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if let Some(directory) = id.strip_prefix("codex:") {
        let home = home_dir(app)?;
        let directory = PathBuf::from(directory);
        if additional_codex_directory(&home, id).is_some() {
            return Ok(directory.join("skills").join(SKILL_NAME));
        }
    }
    targets(app)?
        .into_iter()
        .find(|(candidate, _, _)| *candidate == id)
        .map(|(_, _, path)| path)
        .ok_or_else(|| format!("Unsupported skill target: {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_retires_legacy_skill_without_losing_custom_content() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "new skill").unwrap();
        let legacy = dir.path().join("ctx-ppteer");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("SKILL.md"), "custom instructions").unwrap();
        let destination = dir.path().join(SKILL_NAME);
        install_at(&source, &destination).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "new skill"
        );
        assert!(!legacy.join("SKILL.md").exists());
        let backup = fs::read_dir(&legacy)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(fs::read_to_string(backup).unwrap(), "custom instructions");
        install_at(&source, &destination).unwrap();
        assert_eq!(fs::read_dir(legacy).unwrap().count(), 1);
    }

    #[test]
    fn hashes_skill_contents_and_paths() {
        let directory = tempfile::tempdir().unwrap();
        let skill = directory.path().join("skill");
        fs::create_dir_all(skill.join("nested")).unwrap();
        fs::write(skill.join("SKILL.md"), "initial").unwrap();
        fs::write(skill.join("nested/notes.md"), "notes").unwrap();
        let original = skill_hash(&skill).unwrap();
        fs::write(skill.join("nested/notes.md"), "updated notes").unwrap();
        assert_ne!(original, skill_hash(&skill).unwrap());
        fs::rename(skill.join("nested"), skill.join("renamed")).unwrap();
        assert_ne!(original, skill_hash(&skill).unwrap());
    }
    #[test]
    fn replacements_keep_customized_backup_and_rollback_failed_swap() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("sideglance");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "new").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("SKILL.md"), "custom").unwrap();
        fs::write(destination.join("notes.txt"), "keep").unwrap();
        let mut calls = 0;
        let error = install_with_rename(&source, &destination, |from, to| {
            calls += 1;
            if calls == 2 {
                return Err(std::io::Error::other("injected rename failure"));
            }
            fs::rename(from, to)
        })
        .unwrap_err();
        assert!(error.contains("Previous installation preserved"));
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "custom"
        );
        assert_eq!(
            fs::read_to_string(destination.join("notes.txt")).unwrap(),
            "keep"
        );
        install_at(&source, &destination).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "new"
        );
        let backup = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".sideglance.backup-")
            })
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(backup.join("notes.txt")).unwrap(),
            "keep"
        );
        assert_eq!(
            fs::read_to_string(backup.join("SKILL.md")).unwrap(),
            "custom"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_copy_and_symlink_destination_preserve_originals() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("sideglance");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "new").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("SKILL.md"), "original").unwrap();
        symlink("missing", source.join("unsafe")).unwrap();
        assert!(install_at(&source, &destination).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "original"
        );
        fs::remove_file(source.join("unsafe")).unwrap();
        let linked_destination = dir.path().join("linked");
        symlink(&destination, &linked_destination).unwrap();
        assert!(install_at(&source, &linked_destination).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "original"
        );
    }
}
