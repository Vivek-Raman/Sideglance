use std::{env, path::PathBuf};

use tauri::{AppHandle, Manager};

/// Shared by the desktop app and headless MCP, without needing app startup.
pub fn status_path() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or("Unable to find the home directory")?;
    migrated_path(&PathBuf::from(home), ".local/share", "status.json")
}

pub(crate) fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("Unable to find home directory: {error}"))?;
    migrated_path(&home, ".config", "settings.json")
}

pub(crate) fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|error| format!("Unable to find the home directory: {error}"))
}

// Copy once, never overwrite a Sideglance file or remove the old installation's data.
fn migrated_path(home: &std::path::Path, root: &str, file: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let destination = home.join(root).join("sideglance").join(file);
    if destination.exists() {
        return Ok(destination);
    }
    let legacy = home.join(root).join("ctx-ppteer").join(file);
    let contents = match std::fs::read(&legacy) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(destination),
        Err(error) => return Err(format!("Unable to migrate {}: {error}", legacy.display())),
    };
    let parent = destination.parent().unwrap();
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(&contents).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    match temporary.persist_noclobber(&destination) {
        Ok(_) => {}
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(format!(
                "Unable to migrate {}: {error}",
                destination.display()
            ))
        }
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_originals_and_never_overwrites_new_data() {
        let home = tempfile::tempdir().unwrap();
        for (root, file) in [
            (".config", "settings.json"),
            (".local/share", "status.json"),
        ] {
            let old = home.path().join(root).join("ctx-ppteer").join(file);
            std::fs::create_dir_all(old.parent().unwrap()).unwrap();
            std::fs::write(&old, "original").unwrap();
            let new = migrated_path(home.path(), root, file).unwrap();
            assert_eq!(std::fs::read_to_string(&new).unwrap(), "original");
            std::fs::write(&new, "updated").unwrap();
            migrated_path(home.path(), root, file).unwrap();
            assert_eq!(std::fs::read_to_string(new).unwrap(), "updated");
            assert_eq!(std::fs::read_to_string(old).unwrap(), "original");
        }
    }
}
