use serde_json::{json, Map, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    str::FromStr,
    thread,
    time::{Duration, Instant},
};
use toml_edit::{value, Array, DocumentMut};

use crate::SKILL_NAME;

pub(crate) fn codex_mcp_registered_at(path: &Path) -> Result<bool, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    Ok(document_has_codex_mcp(&document))
}

pub(crate) fn document_has_codex_mcp(document: &DocumentMut) -> bool {
    document
        .as_table()
        .get("mcp_servers")
        .is_some_and(|servers| {
            servers
                .as_table()
                .and_then(|servers| servers.get(SKILL_NAME))
                .is_some_and(|server| server.is_table() || server.is_inline_table())
                || servers
                    .as_inline_table()
                    .and_then(|servers| servers.get(SKILL_NAME))
                    .is_some_and(|server| server.is_inline_table())
        })
}

struct ConfigLock(fs::File);
impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}
fn lock_config(path: &Path) -> Result<ConfigLock, String> {
    use fs2::FileExt;
    let parent = path.parent().ok_or("Configuration directory unavailable")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .ok_or("Configuration name unavailable")?
        .to_string_lossy();
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join(format!(".{name}.sideglance.lock")))
        .map_err(|error| error.to_string())?;
    file.try_lock_exclusive().map_err(|_| {
        "Another Sideglance configuration edit is in progress; retry shortly".to_string()
    })?;
    Ok(ConfigLock(file))
}

pub(crate) fn register_codex_mcp_at(path: &Path, executable: &Path) -> Result<(), String> {
    let _lock = lock_config(path)?;
    let directory = path
        .parent()
        .ok_or("Codex configuration directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let original = read_config_bytes(path)?;
    let contents = String::from_utf8(original.clone().unwrap_or_default())
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    let mut document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    if let Some(servers) = document
        .get_mut("mcp_servers")
        .and_then(|v| v.as_table_like_mut())
    {
        servers.remove("ctx-ppteer");
    }
    document["mcp_servers"][SKILL_NAME]["command"] =
        value(executable.to_string_lossy().to_string());
    document["mcp_servers"][SKILL_NAME]["args"] = value(Array::from_iter(["--mcp"]));
    document["mcp_servers"][SKILL_NAME]["enabled"] = value(true);
    if let Some(server) = document["mcp_servers"][SKILL_NAME].as_table_like_mut() {
        server.remove("disabled");
        server.remove("url");
        server.remove("bearer_token_env_var");
    }
    save_toml_config(path, &document, original.as_deref())
}

fn save_toml_config(
    path: &Path,
    document: &DocumentMut,
    original: Option<&[u8]>,
) -> Result<(), String> {
    save_config(path, document.to_string().as_bytes(), original)
}

pub(crate) fn register_codex_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    register_codex_mcp_at(path, executable)
}

pub(crate) fn unregister_codex_mcp_at(path: &Path) -> Result<(), String> {
    let _lock = lock_config(path)?;
    let original = read_config_bytes(path)?;
    let Some(bytes) = original.as_ref() else {
        return Ok(());
    };
    let contents = std::str::from_utf8(bytes)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    let mut document = DocumentMut::from_str(contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    let removed = document
        .as_table_mut()
        .get_mut("mcp_servers")
        .is_some_and(|servers| {
            servers
                .as_table_mut()
                .is_some_and(|servers| servers.remove(SKILL_NAME).is_some())
                || servers
                    .as_inline_table_mut()
                    .is_some_and(|servers| servers.remove(SKILL_NAME).is_some())
        });
    if removed {
        save_toml_config(path, &document, original.as_deref())?;
    }
    Ok(())
}

fn read_json_config(path: &Path) -> Result<Value, String> {
    match fs::read(path) {
        Ok(contents) if contents.iter().all(u8::is_ascii_whitespace) => Ok(json!({})),
        Ok(contents) => serde_json::from_slice(&contents)
            .map_err(|error| format!("Unable to read {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(error) => Err(format!("Unable to read {}: {error}", path.display())),
    }
}

fn object_mut<'a>(value: &'a mut Value, label: &str) -> Result<&'a mut Map<String, Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| format!("{label} must contain a JSON object"))
}

fn nested_object_mut<'a>(
    root: &'a mut Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    let mut current = object_mut(root, label)?;
    for key in keys {
        let entry = current.entry(*key).or_insert_with(|| json!({}));
        current = object_mut(entry, label)?;
    }
    Ok(current)
}

fn existing_nested_object_mut<'a>(
    value: &'a mut Value,
    keys: &[&str],
) -> Option<&'a mut Map<String, Value>> {
    let (key, remaining) = keys.split_first()?;
    let object = value.as_object_mut()?;
    let next = object.get_mut(*key)?;
    if remaining.is_empty() {
        next.as_object_mut()
    } else {
        existing_nested_object_mut(next, remaining)
    }
}

fn save_json_config(path: &Path, value: &Value, original: Option<&[u8]>) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Unable to encode {}: {error}", path.display()))?;
    save_config(path, &contents, original)
}

fn read_config_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!(
                "{} must be a regular configuration file",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Unable to inspect {}: {error}", path.display())),
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))
}

fn read_json_snapshot(path: &Path) -> Result<(Value, Option<Vec<u8>>), String> {
    let original = read_config_bytes(path)?;
    let value = match original.as_deref() {
        None => json!({}),
        Some(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => json!({}),
        Some(bytes) => serde_json::from_slice(bytes)
            .map_err(|error| format!("Unable to read {}: {error}", path.display()))?,
    };
    Ok((value, original))
}

fn save_config(path: &Path, contents: &[u8], original: Option<&[u8]>) -> Result<(), String> {
    let directory = path.parent().ok_or("Configuration directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("Unable to inspect {}: {error}", path.display())),
    };
    // New files are private, and the temporary file stays private while writing.
    let mut temporary = tempfile::NamedTempFile::new_in(directory)
        .map_err(|error| format!("Unable to create temporary config: {error}"))?;
    temporary
        .write_all(contents)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .map_err(|error| {
                format!(
                    "Unable to preserve permissions for {}: {error}",
                    path.display()
                )
            })?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Unable to sync {}: {error}", path.display()))?;
    // Detect edits since the transaction began. External applications do not
    // share our lock, so a small check-to-rename race remains unavoidable.
    if read_config_bytes(path)?.as_deref() != original {
        return Err(format!(
            "{} changed while Sideglance was editing it. Retry the operation.",
            path.display()
        ));
    }
    temporary
        .persist(path)
        .map_err(|error| format!("Unable to save {}: {error}", path.display()))?;
    Ok(())
}

pub(crate) fn register_standard_json_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    let _lock = lock_config(path)?;
    let (mut config, original) = read_json_snapshot(path)?;
    let servers = nested_object_mut(&mut config, &["mcpServers"], &path.display().to_string())?;
    servers.remove("ctx-ppteer");
    servers.insert(
        SKILL_NAME.into(),
        json!({ "command": executable, "args": ["--mcp"] }),
    );
    save_json_config(path, &config, original.as_deref())
}

pub(crate) fn register_opencode_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    let _lock = lock_config(path)?;
    let (mut config, original) = read_json_snapshot(path)?;
    let servers = nested_object_mut(
        &mut config,
        &["mcp", "servers"],
        &path.display().to_string(),
    )?;
    servers.remove("ctx-ppteer");
    servers.insert(
        SKILL_NAME.into(),
        json!({ "type": "local", "command": [executable, "--mcp"], "codemode": false }),
    );
    save_json_config(path, &config, original.as_deref())
}

pub(crate) fn unregister_json_mcp(path: &Path, keys: &[&str]) -> Result<(), String> {
    let _lock = lock_config(path)?;
    let (mut config, original) = read_json_snapshot(path)?;
    let removed = existing_nested_object_mut(&mut config, keys)
        .is_some_and(|servers| servers.remove(SKILL_NAME).is_some());
    if removed {
        save_json_config(path, &config, original.as_deref())?;
    }
    Ok(())
}

pub(crate) fn json_mcp_registered(path: &Path, keys: &[&str]) -> Result<bool, String> {
    let config = read_json_config(path)?;
    let mut value = &config;
    for key in keys {
        let Some(next) = value.get(key) else {
            return Ok(false);
        };
        value = next;
    }
    Ok(value.get(SKILL_NAME).is_some())
}

pub(crate) fn command_exists(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|directory| {
        let path = directory.join(command);
        path.is_file() || (cfg!(windows) && directory.join(format!("{command}.exe")).is_file())
    })
}

// Temporary output files avoid pipe deadlocks when a CLI emits more than a
// pipe buffer. Only a bounded stderr excerpt reaches the user.
fn run_bounded(command: &mut Command, timeout: Duration) -> std::io::Result<Output> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            result => {
                let error = result.err().unwrap_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Claude Code timed out; retry after checking its installation",
                    )
                });
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    };
    use std::io::{Seek, SeekFrom};
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    let mut output = Output {
        status,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    stdout.take(64 * 1024).read_to_end(&mut output.stdout)?;
    stderr.take(64 * 1024).read_to_end(&mut output.stderr)?;
    Ok(output)
}

pub(crate) fn install_claude_mcp(executable: &Path, config_path: &Path) -> Result<bool, String> {
    let output = match run_bounded(
        Command::new("claude")
            .args(["mcp", "add", SKILL_NAME, "--scope", "user", "--"])
            .arg(executable)
            .arg("--mcp"),
        Duration::from_secs(30),
    ) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to run Claude Code: {error}")),
    };
    if output.status.success() {
        let _lock = lock_config(config_path)?;
        let (mut config, original) = read_json_snapshot(config_path)?;
        if existing_nested_object_mut(&mut config, &["mcpServers"])
            .is_some_and(|servers| servers.remove("ctx-ppteer").is_some())
        {
            save_json_config(config_path, &config, original.as_deref())?;
        }
        Ok(true)
    } else {
        Err(format!(
            "Claude Code rejected the MCP configuration: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(crate) fn uninstall_claude_mcp() -> Result<bool, String> {
    let output = match run_bounded(
        Command::new("claude").args(["mcp", "remove", SKILL_NAME, "--scope", "user"]),
        Duration::from_secs(30),
    ) {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to run Claude Code: {error}")),
    };
    if output.status.success() {
        Ok(true)
    } else {
        Err(format!(
            "Claude Code rejected removal of the MCP configuration: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn registration_replaces_legacy_keys_and_preserves_other_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        for contents in [
            "[mcp_servers.ctx-ppteer]\ncommand = 'old'\n[mcp_servers.other]\ncommand = 'other'\n",
            "mcp_servers = { ctx-ppteer = { command = 'old' }, other = { command = 'other' } }",
        ] {
            fs::write(&path, contents).unwrap();
            register_codex_mcp_at(&path, Path::new("sideglance")).unwrap();
            let saved = fs::read_to_string(&path).unwrap();
            assert!(!saved.contains("ctx-ppteer"));
            assert!(saved.contains("other"));
            assert!(codex_mcp_registered_at(&path).unwrap());
        }
        for keys in [vec!["mcpServers"], vec!["mcp", "servers"]] {
            let mut config = json!({});
            let servers = nested_object_mut(&mut config, &keys, "test").unwrap();
            servers.insert("ctx-ppteer".into(), json!({"command":"old"}));
            servers.insert("other".into(), json!({"command":"other"}));
            save_json_config(&path, &config, read_config_bytes(&path).unwrap().as_deref()).unwrap();
            if keys.len() == 1 {
                register_standard_json_mcp(&path, Path::new("sideglance")).unwrap();
            } else {
                register_opencode_mcp(&path, Path::new("sideglance")).unwrap();
            }
            let saved = fs::read_to_string(&path).unwrap();
            assert!(!saved.contains("ctx-ppteer"));
            assert!(saved.contains("other"));
            assert!(json_mcp_registered(&path, &keys).unwrap());
        }
    }

    #[cfg(unix)]
    #[test]
    fn mcp_changes_preserve_existing_config_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let executable = Path::new("/Applications/sideglance");
        for mode in [0o600, 0o640] {
            let toml = directory.path().join("config.toml");
            fs::write(&toml, "# existing configuration\n").unwrap();
            fs::set_permissions(&toml, fs::Permissions::from_mode(mode)).unwrap();
            register_codex_mcp_at(&toml, executable).unwrap();
            assert_eq!(
                fs::metadata(&toml).unwrap().permissions().mode() & 0o777,
                mode
            );
            unregister_codex_mcp_at(&toml).unwrap();
            assert_eq!(
                fs::metadata(&toml).unwrap().permissions().mode() & 0o777,
                mode
            );

            for opencode in [false, true] {
                let json = directory.path().join("config.json");
                fs::write(&json, "{}").unwrap();
                fs::set_permissions(&json, fs::Permissions::from_mode(mode)).unwrap();
                let keys: &[&str] = if opencode {
                    register_opencode_mcp(&json, executable).unwrap();
                    &["mcp", "servers"]
                } else {
                    register_standard_json_mcp(&json, executable).unwrap();
                    &["mcpServers"]
                };
                assert_eq!(
                    fs::metadata(&json).unwrap().permissions().mode() & 0o777,
                    mode
                );
                unregister_json_mcp(&json, keys).unwrap();
                assert_eq!(
                    fs::metadata(&json).unwrap().permissions().mode() & 0o777,
                    mode
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn new_mcp_configs_are_private_and_ignore_predictable_temp_paths() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let sentinel = directory.path().join("sentinel");
        fs::write(&sentinel, "untouched").unwrap();
        let executable = Path::new("/Applications/sideglance");
        let toml = directory.path().join("config.toml");
        let json = directory.path().join("config.json");
        for extension in ["toml.tmp", "json.tmp"] {
            symlink(
                &sentinel,
                directory.path().join(format!("config.{extension}")),
            )
            .unwrap();
        }
        register_codex_mcp_at(&toml, executable).unwrap();
        register_standard_json_mcp(&json, executable).unwrap();
        for path in [&toml, &json] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(fs::read_to_string(sentinel).unwrap(), "untouched");
    }

    #[test]
    fn recognizes_missing_codex_mcp_servers_table() {
        let document = DocumentMut::from_str("").unwrap();
        assert!(!document_has_codex_mcp(&document));
    }

    #[test]
    fn recognizes_inline_codex_mcp_server() {
        let document = DocumentMut::from_str(
            "[mcp_servers]\nsideglance = { command = \"/Applications/sideglance\", args = [\"--mcp\"] }\n",
        )
        .unwrap();
        assert!(document_has_codex_mcp(&document));
    }

    #[test]
    fn recognizes_inline_codex_mcp_servers_table() {
        let document = DocumentMut::from_str(
            "mcp_servers = { sideglance = { command = \"/Applications/sideglance\", args = [\"--mcp\"] } }\n",
        )
        .unwrap();
        assert!(document_has_codex_mcp(&document));
    }

    #[test]
    fn registers_into_inline_codex_mcp_servers_table() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(
            &path,
            "mcp_servers = { existing = { command = \"existing\" } }\n",
        )
        .unwrap();
        register_codex_mcp_at(&path, Path::new("/Applications/sideglance")).unwrap();
        let document = DocumentMut::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(document_has_codex_mcp(&document));
        assert!(document
            .as_table()
            .get("mcp_servers")
            .and_then(|servers| servers.as_inline_table())
            .and_then(|servers| servers.get("existing"))
            .is_some());
    }

    #[test]
    fn unregisters_inline_codex_mcp_servers_table() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, "mcp_servers = { sideglance = { command = \"sideglance\" }, existing = { command = \"existing\" } }\n").unwrap();
        unregister_codex_mcp_at(&path).unwrap();
        let document = DocumentMut::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!document_has_codex_mcp(&document));
        assert!(document
            .as_table()
            .get("mcp_servers")
            .and_then(|servers| servers.as_inline_table())
            .and_then(|servers| servers.get("existing"))
            .is_some());
    }

    #[test]
    fn registers_and_removes_json_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"existing":{"command":"existing"}}}"#,
        )
        .unwrap();
        register_standard_json_mcp(&path, Path::new("/Applications/sideglance")).unwrap();
        assert!(json_mcp_registered(&path, &["mcpServers"]).unwrap());
        unregister_json_mcp(&path, &["mcpServers"]).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"]["existing"].is_object());
        assert!(config["mcpServers"].get(SKILL_NAME).is_none());
    }

    #[test]
    fn registers_standard_json_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"existing":{"command":"existing"}}}"#,
        )
        .unwrap();
        register_standard_json_mcp(&path, Path::new("/Applications/sideglance")).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"]["existing"].is_object());
        assert_eq!(config["mcpServers"][SKILL_NAME]["args"][0], "--mcp");
    }

    #[test]
    fn treats_an_empty_json_config_as_unconfigured() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(&path, "\n").unwrap();
        assert!(!json_mcp_registered(&path, &["mcpServers"]).unwrap());
        register_standard_json_mcp(&path, Path::new("/Applications/sideglance")).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"][SKILL_NAME].is_object());
    }

    #[test]
    fn unregisters_standard_json_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"sideglance":{"command":"sideglance"},"existing":{"command":"existing"}}}"#,
        )
        .unwrap();
        unregister_json_mcp(&path, &["mcpServers"]).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"]["existing"].is_object());
        assert!(config["mcpServers"].get(SKILL_NAME).is_none());
    }

    #[test]
    fn registers_opencode_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("opencode.json");
        fs::write(
            &path,
            r#"{"mcp":{"servers":{"existing":{"type":"local"}}}}"#,
        )
        .unwrap();
        register_opencode_mcp(&path, Path::new("/Applications/sideglance")).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcp"]["servers"]["existing"].is_object());
        assert_eq!(config["mcp"]["servers"][SKILL_NAME]["type"], "local");
    }
    #[test]
    fn detects_external_config_edit_before_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, r#"{"before":true}"#).unwrap();
        let original = read_config_bytes(&path).unwrap();
        let newer = br#"{"external":"keep me"}"#;
        fs::write(&path, newer).unwrap();
        assert!(save_config(&path, b"{}", original.as_deref())
            .unwrap_err()
            .contains("changed"));
        assert_eq!(fs::read(&path).unwrap(), newer);
        fs::remove_file(&path).unwrap();
        assert!(save_config(&path, b"{}", original.as_deref()).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn cooperating_config_edits_do_not_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let lock = lock_config(&path).unwrap();
        assert!(register_codex_mcp_at(&path, Path::new("sideglance")).is_err());
        drop(lock);
        register_codex_mcp_at(&path, Path::new("sideglance")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_process_captures_large_output_and_reaps_timeout() {
        let output = run_bounded(
            Command::new("sh").args(["-c", "head -c 100000 /dev/zero >&2"]),
            Duration::from_secs(3),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stderr.len(), 64 * 1024);
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("pid");
        let start = Instant::now();
        let error = run_bounded(
            Command::new("sh")
                .args(["-c", "echo $$ > \"$1\"; exec sleep 30", "sh"])
                .arg(&pid_path),
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(3));
        let pid = fs::read_to_string(pid_path).unwrap();
        assert!(!Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }
}

#[cfg(test)]
mod repair_tests {
    use super::*;
    #[test]
    fn repair_enables_local_codex_and_removes_conflicting_remote_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path,"[mcp_servers.sideglance]\nenabled=false\ndisabled=true\nurl='https://example.com/mcp'\n").unwrap();
        register_codex_mcp_at(&path, Path::new("sideglance")).unwrap();
        let doc = fs::read_to_string(&path)
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(
            doc["mcp_servers"][SKILL_NAME]["enabled"].as_bool(),
            Some(true)
        );
        assert!(doc["mcp_servers"][SKILL_NAME].get("url").is_none());
        assert!(doc["mcp_servers"][SKILL_NAME].get("disabled").is_none());
    }
}
