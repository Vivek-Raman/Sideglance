use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, Manager, State};

use crate::{
    desktop::windows::show_settings_window,
    integrations::config::{
        install_claude_mcp, register_codex_mcp, register_codex_mcp_at, register_opencode_mcp,
        register_standard_json_mcp, uninstall_claude_mcp, unregister_codex_mcp_at,
        unregister_json_mcp,
    },
    integrations::registry::{additional_codex_directory, config_path, descriptor, ConfigAdapter},
    integrations::skills::{self, SkillInstallation},
    integrations::status::{harness_available, integration_status},
    paths,
    preferences::{Settings, SettingsOwner},
    status::snapshot::{update, Snapshot, ViewerState},
};

#[tauri::command]
pub(crate) fn get_snapshot(state: State<'_, Arc<ViewerState>>) -> Snapshot {
    state.snapshot.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn set_point_pinned(
    folder_name: String,
    session_id: Option<String>,
    branch: String,
    updated_at: String,
    point_id: String,
    pinned: bool,
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    state
        .store
        .set_point_pinned(
            &folder_name,
            session_id.as_deref(),
            &branch,
            &updated_at,
            &point_id,
            pinned,
        )
        .map_err(|error| error.to_string())?;
    update(&app, &state);
    Ok(())
}

#[tauri::command]
pub(crate) fn dismiss_point(
    folder_name: String,
    session_id: Option<String>,
    branch: String,
    updated_at: String,
    point_id: Option<String>,
    point_index: Option<usize>,
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    if !state
        .store
        .dismiss(
            &folder_name,
            session_id.as_deref(),
            &branch,
            &updated_at,
            point_id.as_deref().unwrap_or(""),
            point_index,
        )
        .map_err(|error| error.to_string())?
    {
        return Err("This point changed before it could be dismissed. Please try again.".into());
    }
    update(&app, &state);
    Ok(())
}

#[tauri::command]
pub(crate) fn restore_point(
    entry: crate::status::Entry,
    point_index: usize,
    app: AppHandle,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    state
        .store
        .restore(entry, point_index)
        .map_err(|error| error.to_string())?;
    update(&app, &state);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_undo_limit(
    undo_limit: usize,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.update(|settings| {
        settings.undo_limit = undo_limit;
        Ok(())
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_settings(owner: State<'_, Arc<SettingsOwner>>) -> Result<Settings, String> {
    Ok(owner.snapshot())
}

#[tauri::command]
pub(crate) fn reset_settings(
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
    state: State<'_, Arc<ViewerState>>,
) -> Result<Settings, String> {
    let settings = owner.reset()?;
    app.get_webview_window("main")
        .ok_or_else(|| "Main window unavailable".to_string())?
        .set_always_on_top(settings.pinned)
        .map_err(|error| error.to_string())?;
    update(&app, &state);
    Ok(settings)
}

#[tauri::command]
pub(crate) fn open_settings(app: AppHandle) {
    show_settings_window(&app);
}

#[tauri::command]
pub(crate) fn set_appearance_settings(
    theme: String,
    primary_color: String,
    text_scale: f64,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_appearance(&theme, primary_color, text_scale)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_status_grouping(
    status_grouping: crate::preferences::StatusGrouping,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.update(|settings| {
        settings.status_grouping = status_grouping;
        Ok(())
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_project_sort_order(
    project_sort_order: String,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_sort_order(&project_sort_order)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_status_sort_order(
    status_sort_order: String,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_status_sort_order(&status_sort_order)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn set_auto_clear_after_minutes(
    auto_clear_after_minutes: Option<u64>,
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    owner.set_retention(auto_clear_after_minutes)?;
    crate::status::retention::clear_expired_points(&state, &owner)
        .map_err(|error| format!("Auto-clear setting saved, but pruning failed: {error}"))?;
    update(&app, &state);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_max_status_items_per_project(
    max_status_items_per_project: Option<usize>,
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
    state: State<'_, Arc<ViewerState>>,
) -> Result<(), String> {
    owner.update(|settings| {
        settings.max_status_items_per_project = max_status_items_per_project;
        Ok(())
    })?;
    crate::status::retention::clear_expired_points(&state, &owner)
        .map_err(|error| format!("Count setting saved, but pruning failed: {error}"))?;
    update(&app, &state);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_pinned(
    pinned: bool,
    app: AppHandle,
    owner: State<'_, Arc<SettingsOwner>>,
) -> Result<(), String> {
    owner.set_pinned(pinned)?;
    app.get_webview_window("main")
        .ok_or_else(|| "Main window unavailable".to_string())?
        .set_always_on_top(pinned)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn restart_app(app: AppHandle) {
    app.restart();
}

#[tauri::command]
pub(crate) fn get_skill_installation(app: AppHandle) -> Result<SkillInstallation, String> {
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn install_skill(app: AppHandle) -> Result<SkillInstallation, String> {
    let source = skills::bundled_source(&app)?;
    for (_, _, destination) in skills::targets(&app)? {
        skills::install_at(&source, &destination)?;
    }
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn install_skill_for_target(
    target: String,
    app: AppHandle,
) -> Result<SkillInstallation, String> {
    let source = skills::bundled_source(&app)?;
    let destination = skills::target(&app, &target)?;
    skills::install_at(&source, &destination)?;
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn install_mcp_for_harness(
    harness: String,
    app: AppHandle,
) -> Result<SkillInstallation, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Unable to locate the Sideglance executable: {error}"))?;
    let home = paths::home_dir(&app)?;
    if let Some(directory) = harness.strip_prefix("codex:") {
        let directory = PathBuf::from(directory);
        if additional_codex_directory(&home, &harness).is_some() {
            register_codex_mcp_at(&directory.join("config.toml"), &executable)?;
            return integration_status(&app);
        }
    }
    if !harness_available(&harness, &home) {
        return Err(format!("{harness} was not found on this computer"));
    }
    let agent = descriptor(&harness).ok_or_else(|| format!("Unsupported harness: {harness}"))?;
    if !agent.supports_mcp {
        return Err(format!("{harness} does not support MCP installation"));
    }
    match agent.adapter {
        ConfigAdapter::CodexToml => register_codex_mcp(&config_path(&home, agent), &executable)?,
        ConfigAdapter::ClaudeCli => {
            if !install_claude_mcp(&executable, &config_path(&home, agent))? {
                return Err(
                    "Claude Code is not installed; its MCP tool was not registered.".into(),
                );
            }
        }
        ConfigAdapter::StandardJson => {
            register_standard_json_mcp(&config_path(&home, agent), &executable)?
        }
        ConfigAdapter::OpenCodeJson => {
            register_opencode_mcp(&config_path(&home, agent), &executable)?
        }
    }
    integration_status(&app)
}

#[tauri::command]
pub(crate) fn uninstall_mcp_for_harness(
    harness: String,
    app: AppHandle,
) -> Result<SkillInstallation, String> {
    let home = paths::home_dir(&app)?;
    if let Some(directory) = harness.strip_prefix("codex:") {
        let directory = PathBuf::from(directory);
        if additional_codex_directory(&home, &harness).is_some() {
            unregister_codex_mcp_at(&directory.join("config.toml"))?;
            return integration_status(&app);
        }
    }
    if !harness_available(&harness, &home) {
        return Err(format!("{harness} was not found on this computer"));
    }
    let agent = descriptor(&harness).ok_or_else(|| format!("Unsupported harness: {harness}"))?;
    if !agent.supports_mcp {
        return Err(format!("{harness} does not support MCP removal"));
    }
    match agent.adapter {
        ConfigAdapter::CodexToml => {
            crate::integrations::config::unregister_codex_mcp_at(&config_path(&home, agent))?
        }
        ConfigAdapter::ClaudeCli => {
            if !uninstall_claude_mcp()? {
                return Err("Claude Code is not installed; its MCP tool was not removed.".into());
            }
        }
        ConfigAdapter::StandardJson => {
            unregister_json_mcp(&config_path(&home, agent), &["mcpServers"])?
        }
        ConfigAdapter::OpenCodeJson => {
            unregister_json_mcp(&config_path(&home, agent), &["mcp", "servers"])?
        }
    }
    integration_status(&app)
}
