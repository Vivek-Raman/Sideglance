use serde_json::{json, Map, Value};
use std::{fs, io::Write, path::Path, process::Command, str::FromStr};
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

pub(crate) fn register_codex_mcp_at(path: &Path, executable: &Path) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or("Codex configuration directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let mut document = DocumentMut::from_str(&contents)
        .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
    document["mcp_servers"][SKILL_NAME]["command"] =
        value(executable.to_string_lossy().to_string());
    document["mcp_servers"][SKILL_NAME]["args"] = value(Array::from_iter(["--mcp"]));
    save_toml_config(path, &document)
}

fn save_toml_config(path: &Path, document: &DocumentMut) -> Result<(), String> {
    save_config(path, document.to_string().as_bytes())
}

pub(crate) fn register_codex_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    register_codex_mcp_at(path, executable)
}

pub(crate) fn unregister_codex_mcp_at(path: &Path) -> Result<(), String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
    };
    let mut document = DocumentMut::from_str(&contents)
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
        save_toml_config(path, &document)?;
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

fn save_json_config(path: &Path, value: &Value) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Unable to encode {}: {error}", path.display()))?;
    save_config(path, &contents)
}

fn save_config(path: &Path, contents: &[u8]) -> Result<(), String> {
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
    temporary
        .persist(path)
        .map_err(|error| format!("Unable to save {}: {error}", path.display()))?;
    Ok(())
}

pub(crate) fn register_standard_json_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    let mut config = read_json_config(path)?;
    let servers = nested_object_mut(&mut config, &["mcpServers"], &path.display().to_string())?;
    servers.insert(
        SKILL_NAME.into(),
        json!({ "command": executable, "args": ["--mcp"] }),
    );
    save_json_config(path, &config)
}

pub(crate) fn register_opencode_mcp(path: &Path, executable: &Path) -> Result<(), String> {
    let mut config = read_json_config(path)?;
    let servers = nested_object_mut(
        &mut config,
        &["mcp", "servers"],
        &path.display().to_string(),
    )?;
    servers.insert(
        SKILL_NAME.into(),
        json!({ "type": "local", "command": [executable, "--mcp"], "codemode": false }),
    );
    save_json_config(path, &config)
}

pub(crate) fn unregister_json_mcp(path: &Path, keys: &[&str]) -> Result<(), String> {
    let mut config = read_json_config(path)?;
    let removed = existing_nested_object_mut(&mut config, keys)
        .is_some_and(|servers| servers.remove(SKILL_NAME).is_some());
    if removed {
        save_json_config(path, &config)?;
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

pub(crate) fn install_claude_mcp(executable: &Path) -> Result<bool, String> {
    let output = match Command::new("claude")
        .args(["mcp", "add", SKILL_NAME, "--scope", "user", "--"])
        .arg(executable)
        .arg("--mcp")
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("Unable to run Claude Code: {error}")),
    };
    if output.status.success() {
        Ok(true)
    } else {
        Err(format!(
            "Claude Code rejected the MCP configuration: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(crate) fn uninstall_claude_mcp() -> Result<bool, String> {
    let output = match Command::new("claude")
        .args(["mcp", "remove", SKILL_NAME, "--scope", "user"])
        .output()
    {
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
    fn mcp_changes_preserve_existing_config_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let executable = Path::new("/Applications/ctx-ppteer");
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
        let executable = Path::new("/Applications/ctx-ppteer");
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
            "[mcp_servers]\nctx-ppteer = { command = \"/Applications/ctx-ppteer\", args = [\"--mcp\"] }\n",
        )
        .unwrap();
        assert!(document_has_codex_mcp(&document));
    }

    #[test]
    fn recognizes_inline_codex_mcp_servers_table() {
        let document = DocumentMut::from_str(
            "mcp_servers = { ctx-ppteer = { command = \"/Applications/ctx-ppteer\", args = [\"--mcp\"] } }\n",
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
        register_codex_mcp_at(&path, Path::new("/Applications/ctx-ppteer")).unwrap();
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
        fs::write(&path, "mcp_servers = { ctx-ppteer = { command = \"ctx-ppteer\" }, existing = { command = \"existing\" } }\n").unwrap();
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
        register_standard_json_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();
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
        register_standard_json_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();
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
        register_standard_json_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcpServers"][SKILL_NAME].is_object());
    }

    #[test]
    fn unregisters_standard_json_mcp_without_losing_existing_servers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"ctx-ppteer":{"command":"ctx-ppteer"},"existing":{"command":"existing"}}}"#,
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
        register_opencode_mcp(&path, Path::new("/Applications/ctx-ppteer")).unwrap();
        let config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(config["mcp"]["servers"]["existing"].is_object());
        assert_eq!(config["mcp"]["servers"][SKILL_NAME]["type"], "local");
    }
}
