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
            if file_type.is_symlink() {
                return Err(format!(
                    "Cannot hash skill containing symbolic link: {}",
                    path.display()
                ));
            }
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
            } else {
                return Err(format!("Unsupported skill file: {}", path.display()));
            }
        }
        Ok(())
    }
    let mut hasher = Sha256::new();
    hash_directory(path, path, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

// Explicitly unlock before closing: a concurrent subprocess can briefly inherit
// the open file description on Unix and otherwise keep its lock alive.
struct InstallationLock(fs::File);

impl Drop for InstallationLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

fn installation_lock_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::WouldBlock
        || error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
    {
        format!("Another skill installation is in progress; retry shortly: {error}")
    } else {
        format!("Unable to lock skill installation: {error}")
    }
}

/// Resolve parent aliases without accepting a symlink as the skill itself.
pub(crate) fn resolved_destination(destination: &Path) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or("Skill destination has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    Ok(fs::canonicalize(parent).map_err(|e| e.to_string())?.join(
        destination
            .file_name()
            .ok_or("Skill destination has no name")?,
    ))
}

pub(crate) fn install_at(
    source: &Path,
    destination: &Path,
    storage: &Path,
) -> Result<bool, String> {
    install_with_rename(source, destination, storage, |from, to| {
        fs::rename(from, to)
    })
}

fn save_record(path: &Path, contents: &str) -> Result<(), String> {
    use std::io::Write;
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(|e| e.to_string())?;
    file.write_all(contents.as_bytes())
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_destination(destination: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) => {
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || !destination.join("SKILL.md").is_file()
            {
                return Err(format!("{} already exists and is not a regular skill directory. Remove or rename it before installing.", destination.display()));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

// Unknown or edited copies are archived once per content hash. Managed copies
// only need to survive until the replacement has landed.
fn finish_replacement(record: &Path) -> Result<(), String> {
    let backup = record.join("rollback");
    if backup.exists() {
        let hash = skill_hash(&backup)?;
        if fs::read_to_string(record.join("managed-hash"))
            .ok()
            .as_deref()
            == Some(&hash)
        {
            fs::remove_dir_all(&backup).map_err(|e| e.to_string())?;
        } else {
            let archive = record.join(format!("preserved-{hash}"));
            if archive.exists() && skill_hash(&archive)? == hash {
                fs::remove_dir_all(&backup).map_err(|e| e.to_string())?;
            } else {
                fs::rename(&backup, &archive).map_err(|e| e.to_string())?;
            }
        }
    }
    let pending = record.join("pending-hash");
    if pending.exists() {
        let hash = fs::read_to_string(&pending).map_err(|e| e.to_string())?;
        save_record(&record.join("managed-hash"), &hash)?;
        fs::remove_file(pending).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn install_with_rename(
    source: &Path,
    destination: &Path,
    storage: &Path,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<bool, String> {
    use fs2::FileExt;
    let source_type = fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if !source_type.is_dir() || !source.join("SKILL.md").is_file() {
        return Err("The skill source must be a directory containing SKILL.md".into());
    }
    let destination = resolved_destination(destination)?;
    let record = storage.join(format!(
        "{:x}",
        Sha256::digest(destination.as_os_str().as_encoded_bytes())
    ));
    fs::create_dir_all(&record).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(record.join("install.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock_exclusive().map_err(installation_lock_error)?;
    let _lock = InstallationLock(lock);
    // Only this target's lock holder can own staging directories here.
    for entry in fs::read_dir(&record).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with("staging-")
            && entry.file_type().map_err(|e| e.to_string())?.is_dir()
        {
            fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    save_record(&record.join("destination"), &destination.to_string_lossy())?;
    let mut existing = validate_destination(&destination)?;
    let backup = record.join("rollback");
    if backup.exists() && !existing {
        rename(&backup, &destination).map_err(|e| {
            format!(
                "Unable to recover {} from {}: {e}",
                destination.display(),
                backup.display()
            )
        })?;
        let _ = fs::remove_file(record.join("pending-hash"));
        existing = true;
    }
    if !backup.exists() {
        let pending = record.join("pending-hash");
        if pending.exists() {
            let hash = fs::read_to_string(&pending).map_err(|e| e.to_string())?;
            if !existing || skill_hash(&destination)? != hash {
                fs::remove_file(&pending).map_err(|e| e.to_string())?;
            }
        }
    }
    // A present destination means the publish rename completed. Keep any
    // subsequent external edits; the recorded hash remains the published hash.
    finish_replacement(&record)?;
    retire_legacy_skill(&destination, &record)?;
    let stage = tempfile::Builder::new()
        .prefix("staging-")
        .tempdir_in(&record)
        .map_err(|e| e.to_string())?;
    copy_directory(source, stage.path())?;
    let hash = skill_hash(stage.path())?;
    if skill_hash(source)? != hash {
        return Err("Skill source changed during installation; retry the operation".into());
    }
    if existing && skill_hash(&destination)? == hash {
        // Do not adopt unknown copies as managed just because they match today.
        return Ok(false);
    }
    save_record(&record.join("pending-hash"), &hash)?;
    if existing {
        rename(&destination, &backup).map_err(|e| {
            format!(
                "Unable to prepare {} for replacement: {e}",
                destination.display()
            )
        })?;
    }
    if let Err(error) = rename(stage.path(), &destination) {
        if existing {
            rename(&backup, &destination).map_err(|restore_error| format!(
                "Installation failed: {error}. Unable to restore previous skill: {restore_error}. Recover it from {}", backup.display()))?;
        }
        fs::remove_file(record.join("pending-hash")).map_err(|e| e.to_string())?;
        return Err(format!(
            "Unable to install {}: {error}. Previous installation preserved.",
            destination.display()
        ));
    }
    finish_replacement(&record)
        .map_err(|e| format!("Skill installed, but recovery cleanup failed; retry setup: {e}"))?;
    Ok(true)
}

// Migration runs before replacement, including on an otherwise unchanged install.
fn retire_legacy_skill(destination: &Path, record: &Path) -> Result<(), String> {
    if destination.file_name().and_then(|name| name.to_str()) != Some(SKILL_NAME) {
        return Ok(());
    }
    let legacy = destination.with_file_name("ctx-ppteer");
    if legacy.join("SKILL.md").is_file() {
        if fs::symlink_metadata(&legacy)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err(format!(
                "Legacy skill {} is a symlink; move it outside the skill directory before retrying",
                legacy.display()
            ));
        }
        let archive = record.join(format!("legacy-{}", uuid::Uuid::new_v4()));
        fs::rename(&legacy, &archive)
            .map_err(|e| format!("Unable to archive legacy skill {}: {e}", legacy.display()))?;
    }
    Ok(())
}

pub(crate) fn installation_storage(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(home_dir(app)?.join(".local/share/sideglance/skill-installations"))
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
        install_at(&source, &destination, &dir.path().join("recovery")).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "new skill"
        );
        assert!(!legacy.join("SKILL.md").exists());
        let record = fs::read_dir(dir.path().join("recovery"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let archive = fs::read_dir(&record)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().starts_with("legacy-"))
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(archive.join("SKILL.md")).unwrap(),
            "custom instructions"
        );
        assert!(!install_at(&source, &destination, &dir.path().join("recovery")).unwrap());
        assert_eq!(
            fs::read_dir(&record)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with("legacy-"))
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn installation_unlocks_even_with_another_open_descriptor() {
        use fs2::FileExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("install.lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.try_lock_exclusive().unwrap();
        // Models the shared file description inherited during a concurrent fork.
        let inherited = file.try_clone().unwrap();
        let guard = InstallationLock(file);
        let contender = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(contender.try_lock_exclusive().is_err());
        drop(guard);
        contender.try_lock_exclusive().unwrap();
        FileExt::unlock(&contender).unwrap();
        drop(inherited);
    }

    #[test]
    fn installation_lock_errors_distinguish_contention() {
        let contention =
            installation_lock_error(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert!(contention.contains("Another skill installation"));
        assert!(installation_lock_error(fs2::lock_contended_error())
            .contains("Another skill installation"));
        let other = installation_lock_error(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "test lock permission failure",
        ));
        assert!(other.contains("Unable to lock skill installation"));
        assert!(other.contains("test lock permission failure"));
        assert!(!other.contains("Another skill installation"));
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
        let error = install_with_rename(
            &source,
            &destination,
            &dir.path().join("recovery"),
            |from, to| {
                calls += 1;
                if calls == 2 {
                    return Err(std::io::Error::other("injected rename failure"));
                }
                fs::rename(from, to)
            },
        )
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
        install_at(&source, &destination, &dir.path().join("recovery")).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "new"
        );
        let record = fs::read_dir(dir.path().join("recovery"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let backup = fs::read_dir(record)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("preserved-")
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

    fn record_in(storage: &Path) -> PathBuf {
        fs::read_dir(storage)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }

    #[test]
    fn unchanged_and_managed_updates_do_not_accumulate_backups() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("skills/sideglance");
        let storage = dir.path().join("recovery");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "v1").unwrap();
        assert!(install_at(&source, &destination, &storage).unwrap());
        let abandoned = record_in(&storage).join("staging-abandoned");
        fs::create_dir(&abandoned).unwrap();
        fs::write(abandoned.join("SKILL.md"), "partial").unwrap();
        assert!(!install_at(&source, &destination, &storage).unwrap());
        assert!(!abandoned.exists());
        fs::write(source.join("SKILL.md"), "v2").unwrap();
        assert!(install_at(&source, &destination, &storage).unwrap());
        let record = record_in(&storage);
        assert_eq!(fs::read_dir(&record).unwrap().count(), 3); // lock, destination, managed hash
        assert_eq!(
            fs::read_dir(destination.parent().unwrap()).unwrap().count(),
            1
        );
        fs::write(destination.join("notes.txt"), "custom").unwrap();
        assert!(install_at(&source, &destination, &storage).unwrap());
        let archive = fs::read_dir(record)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().starts_with("preserved-"))
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(archive.join("notes.txt")).unwrap(),
            "custom"
        );
    }

    #[test]
    fn retry_recovers_when_both_publish_and_restore_failed() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("sideglance");
        let storage = dir.path().join("recovery");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("SKILL.md"), "new").unwrap();
        fs::write(destination.join("SKILL.md"), "custom").unwrap();
        let mut calls = 0;
        let error = install_with_rename(&source, &destination, &storage, |from, to| {
            calls += 1;
            if calls > 1 {
                return Err(std::io::Error::other("injected failure"));
            }
            fs::rename(from, to)
        })
        .unwrap_err();
        assert!(error.contains("Recover it from"));
        assert!(!destination.exists());
        assert!(install_at(&source, &destination, &storage).unwrap());
        let record = record_in(&storage);
        let archive = fs::read_dir(record)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().starts_with("preserved-"))
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(archive.join("SKILL.md")).unwrap(),
            "custom"
        );
    }

    #[test]
    fn retry_finishes_published_transaction_without_another_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("sideglance");
        let storage = dir.path().join("recovery");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "old").unwrap();
        install_at(&source, &destination, &storage).unwrap();
        let record = record_in(&storage);
        fs::write(source.join("SKILL.md"), "new").unwrap();
        save_record(&record.join("pending-hash"), &skill_hash(&source).unwrap()).unwrap();
        fs::rename(&destination, record.join("rollback")).unwrap();
        copy_directory(&source, &destination).unwrap();
        assert!(!install_at(&source, &destination, &storage).unwrap());
        assert!(!record.join("rollback").exists());
        assert!(!record.join("pending-hash").exists());
        assert_eq!(fs::read_dir(record).unwrap().count(), 3);
    }

    #[test]
    fn abandoned_prepare_does_not_mark_old_custom_content_as_managed() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("sideglance");
        let storage = dir.path().join("recovery");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "old").unwrap();
        install_at(&source, &destination, &storage).unwrap();
        let record = record_in(&storage);
        fs::write(destination.join("SKILL.md"), "custom").unwrap();
        fs::write(source.join("SKILL.md"), "new").unwrap();
        save_record(&record.join("pending-hash"), &skill_hash(&source).unwrap()).unwrap();
        install_at(&source, &destination, &storage).unwrap();
        assert!(fs::read_dir(record)
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.file_name().to_string_lossy().starts_with("preserved-")));
    }

    #[cfg(unix)]
    #[test]
    fn parent_aliases_share_installation_and_recovery_state() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("skills/sideglance");
        let storage = dir.path().join("recovery");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), "new").unwrap();
        install_at(&source, &destination, &storage).unwrap();
        symlink(destination.parent().unwrap(), dir.path().join("alias")).unwrap();
        let alias = dir.path().join("alias/sideglance");
        assert_eq!(
            resolved_destination(&alias).unwrap(),
            resolved_destination(&destination).unwrap()
        );
        assert!(!install_at(&source, &alias, &storage).unwrap());
        assert_eq!(fs::read_dir(&storage).unwrap().count(), 1);
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
        assert!(install_at(&source, &destination, &dir.path().join("recovery")).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "original"
        );
        fs::remove_file(source.join("unsafe")).unwrap();
        let linked_destination = dir.path().join("linked");
        symlink(&destination, &linked_destination).unwrap();
        assert!(install_at(&source, &linked_destination, &dir.path().join("recovery")).is_err());
        assert_eq!(
            fs::read_to_string(destination.join("SKILL.md")).unwrap(),
            "original"
        );
    }
}
