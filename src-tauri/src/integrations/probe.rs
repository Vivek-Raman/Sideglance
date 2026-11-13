//! Explicit, read-only MCP connectivity checks. No status write is used as a probe.
use super::registry::{additional_codex_directory, config_path, descriptor, ConfigAdapter};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[derive(Serialize)]
pub struct ConnectionTest {
    pub status: String,
    pub message: String,
}

pub(crate) fn configuration(home: &Path, harness: &str) -> Result<(String, Vec<String>), String> {
    let (path, adapter) = if let Some(directory) = additional_codex_directory(home, harness) {
        (directory.join("config.toml"), ConfigAdapter::CodexToml)
    } else {
        let agent = descriptor(harness).ok_or("Unsupported agent")?;
        (config_path(home, agent), agent.adapter)
    };
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|_| "Agent configuration could not be read")?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Agent configuration could not be read")?;
    if bytes.len() > 1024 * 1024 {
        return Err("Agent configuration exceeds 1 MiB".into());
    }
    let (command, args) = if adapter == ConfigAdapter::CodexToml {
        let text = std::str::from_utf8(&bytes).map_err(|_| "Agent configuration is not UTF-8")?;
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|_| "Agent TOML configuration is invalid")?;
        let server = doc
            .get("mcp_servers")
            .and_then(|servers| servers.get("sideglance"))
            .ok_or("Sideglance MCP is not configured")?;
        if server.get("enabled").and_then(|v| v.as_bool()) == Some(false)
            || server.get("disabled").and_then(|v| v.as_bool()) == Some(true)
        {
            return Err("Sideglance MCP is disabled".into());
        }
        let command = server
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or("MCP executable is missing")?
            .to_string();
        let args = server
            .get("args")
            .and_then(|v| v.as_array())
            .ok_or("MCP arguments are missing")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or("MCP arguments must be strings")
            })
            .collect::<Result<Vec<_>, _>>()?;
        (command, args)
    } else {
        let config: Value =
            serde_json::from_slice(&bytes).map_err(|_| "Agent JSON configuration is invalid")?;
        let server = if adapter == ConfigAdapter::OpenCodeJson {
            config.pointer("/mcp/servers/sideglance")
        } else {
            config.pointer("/mcpServers/sideglance")
        }
        .ok_or("Sideglance MCP is not configured")?;
        if server.get("enabled").and_then(Value::as_bool) == Some(false)
            || server.get("disabled").and_then(Value::as_bool) == Some(true)
        {
            return Err("Sideglance MCP is disabled".into());
        }
        if adapter == ConfigAdapter::OpenCodeJson {
            let command = server
                .get("command")
                .and_then(Value::as_array)
                .ok_or("MCP command is missing")?;
            let mut parts = command
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or("MCP command must contain strings")
                })
                .collect::<Result<Vec<_>, _>>()?;
            if parts.is_empty() {
                return Err("MCP executable is missing".into());
            }
            (parts.remove(0), parts)
        } else {
            let command = server
                .get("command")
                .and_then(Value::as_str)
                .ok_or("MCP executable is missing")?
                .to_string();
            let args = server
                .get("args")
                .and_then(Value::as_array)
                .ok_or("MCP arguments are missing")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or("MCP arguments must be strings")
                })
                .collect::<Result<Vec<_>, _>>()?;
            (command, args)
        }
    };
    if args != ["--mcp"] {
        return Err(
            "Sideglance requires exactly the --mcp argument. Repair the configuration.".into(),
        );
    }
    let executable = resolve_executable(&command)
        .ok_or("MCP executable was not found or is not executable. Repair the configuration.")?;
    Ok((executable.to_string_lossy().into_owned(), args))
}
fn resolve_executable(command: &str) -> Option<PathBuf> {
    let path = PathBuf::from(command);
    let candidates = if path.is_absolute() {
        vec![path]
    } else if path.components().count() == 1 {
        std::env::split_paths(&std::env::var_os("PATH")?)
            .flat_map(|dir| {
                let mut paths = vec![dir.join(command)];
                if cfg!(windows) {
                    paths.push(dir.join(format!("{command}.exe")));
                }
                paths
            })
            .collect()
    } else {
        return None;
    };
    candidates.into_iter().find(|path| {
        let Ok(meta) = fs::metadata(path) else {
            return false;
        };
        if !meta.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}

pub(crate) fn test_connection(home: &Path, harness: &str) -> ConnectionTest {
    let (executable, args) = match configuration(home, harness) {
        Ok(value) => value,
        Err(message) => {
            return ConnectionTest {
                status: "invalid".into(),
                message,
            }
        }
    };
    match handshake(Command::new(executable).args(args), Duration::from_secs(5)) {
        Ok(()) => ConnectionTest { status: "connected".into(), message: "A new connection responded with Sideglance tools. Restart existing agent sessions after an update; this test does not reload them.".into() },
        Err(message) => ConnectionTest { status: "error".into(), message },
    }
}

fn handshake(command: &mut Command, timeout: Duration) -> Result<(), String> {
    let mut child_command = tokio::process::Command::new(command.get_program());
    child_command.args(command.get_args());
    for (name, value) in command.get_envs() {
        if let Some(value) = value {
            child_command.env(name, value);
        } else {
            child_command.env_remove(name);
        }
    }
    if let Some(directory) = command.get_current_dir() {
        child_command.current_dir(directory);
    }
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?.block_on(async {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        let mut child = child_command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn().map_err(|error| format!("Unable to start MCP: {error}"))?;
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap().take(64 * 1024)).lines();
        async fn response<R: tokio::io::AsyncBufRead + Unpin>(output: &mut tokio::io::Lines<R>, id: u64) -> Result<Value, String> {
            while let Some(line) = output.next_line().await.map_err(|e| e.to_string())? {
                let Ok(value) = serde_json::from_str::<Value>(&line) else { continue; };
                if value.get("id").and_then(Value::as_u64) != Some(id) { continue; }
                if value.get("error").is_some() { return Err("MCP rejected the protocol request".into()); }
                return value.get("result").cloned().ok_or_else(|| "MCP returned an invalid response".into());
            }
            Err("MCP output ended or exceeded 64 KiB before returning a valid response".into())
        }
        let result = tokio::time::timeout(timeout, async {
            let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"sideglance-connection-test","version":"1"}}});
            input.write_all(format!("{initialize}\n").as_bytes()).await.map_err(|e| e.to_string())?;
            let initialized = response(&mut output,1).await?;
            if initialized.get("protocolVersion").and_then(Value::as_str) != Some("2024-11-05") { return Err("MCP negotiated an unsupported protocol version".into()); }
            let initialized = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
            let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
            input.write_all(format!("{initialized}\n{list}\n").as_bytes()).await.map_err(|e| e.to_string())?;
            let listed = response(&mut output,2).await?;
            let tools = listed.get("tools").and_then(Value::as_array).ok_or("MCP did not return tools")?;
            if !["read_status","write_status"].iter().all(|name| tools.iter().any(|tool| tool.get("name").and_then(Value::as_str)==Some(name))) { return Err("Connected server does not provide Sideglance tools".into()); }
            Ok(())
        }).await.unwrap_or_else(|_| Err("MCP did not respond before the connection deadline".into()));
        drop(input);
        drop(output);
        let _ = child.kill().await;
        let _ = child.wait().await;
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_reports_disabled_missing_and_wrong_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join(".codex/config.toml");
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        for (contents, expected) in [
            ("[mcp_servers.sideglance]\nenabled=false", "disabled"),
            (
                "[mcp_servers.sideglance]\ncommand='missing-sideglance'\nargs=['--mcp']",
                "not found",
            ),
            (
                "[mcp_servers.sideglance]\ncommand='sideglance'\nargs=[]",
                "--mcp",
            ),
        ] {
            fs::write(&config, contents).unwrap();
            assert!(configuration(dir.path(), "codex")
                .unwrap_err()
                .contains(expected));
        }
    }
    #[cfg(unix)]
    #[test]
    fn actual_stdio_connection_is_read_only_and_timeout_is_bounded() {
        let exe = Path::new("/tmp/sideglance-legacy-remediation-mcp");
        if exe.is_file() {
            let home = tempfile::tempdir().unwrap();
            handshake(
                Command::new(exe).arg("--mcp").env("HOME", home.path()),
                Duration::from_secs(2),
            )
            .unwrap();
            assert!(!home
                .path()
                .join(".local/share/sideglance/status.json")
                .exists());
        }
        let started = std::time::Instant::now();
        assert!(handshake(Command::new("sleep").arg("3"), Duration::from_millis(40)).is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    #[test]
    fn mock_server() {
        use std::io::{BufRead, Write};
        let Ok(mode) = std::env::var("SIDEGLANCE_PROBE_TEST") else {
            return;
        };
        let mut lines = std::io::stdin().lock().lines();
        assert!(lines.next().is_some());
        let version = if mode == "wrong-version" {
            "unsupported"
        } else {
            "2024-11-05"
        };
        println!(
            "{}",
            json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":version}})
        );
        std::io::stdout().flush().unwrap();
        if mode == "wrong-version" {
            return;
        }
        assert!(lines.next().is_some());
        assert!(lines.next().is_some());
        let tools = if mode == "missing-tools" {
            vec![]
        } else {
            vec![
                json!({"name":"read_status"}),
                json!({"name":"write_status"}),
            ]
        };
        println!(
            "{}",
            json!({"jsonrpc":"2.0","id":2,"result":{"tools":tools}})
        );
        std::io::stdout().flush().unwrap();
    }
    #[test]
    fn handshake_checks_negotiation_and_required_tools() {
        for (mode, expected) in [
            ("ok", None),
            ("wrong-version", Some("unsupported")),
            ("missing-tools", Some("does not provide")),
        ] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "integrations::probe::protocol_tests::mock_server",
                    "--nocapture",
                ])
                .env("SIDEGLANCE_PROBE_TEST", mode);
            let result = handshake(&mut command, Duration::from_secs(3));
            if let Some(expected) = expected {
                assert!(result.unwrap_err().contains(expected));
            } else {
                result.unwrap();
            }
        }
    }
}
