use tauri::{
    menu::{Menu, MenuItem},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

/// Call on the main thread before showing a newly created window.
pub(crate) fn restore_native_frame(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWindow;
        use objc2_foundation::NSString;

        // Tauri owns this NSWindow. The borrowed pointer is used only on the
        // main thread while its WebviewWindow is alive.
        let native = unsafe { &*window.ns_window()?.cast::<NSWindow>() };
        let name = NSString::from_str(&format!("sideglance.{}", window.label()));
        // Restore before enabling autosave so the initial frame cannot replace
        // the saved one. AppKit handles persistence and screen adjustment.
        native.setFrameUsingName(&name);
        if !native.setFrameAutosaveName(&name) {
            return Err(std::io::Error::other("Window frame autosave name already in use").into());
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    Ok(())
}

pub(crate) fn install_menu(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle();
    let menu = Menu::default(handle)?;
    let settings = MenuItem::with_id(handle, "settings", "Settings…", true, None::<&str>)?;
    if let Some(application) = menu.items()?.first().and_then(|item| item.as_submenu()) {
        application.insert(&settings, 1)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "settings" {
            if let Err(error) = show_settings_window(app) {
                eprintln!("Failed to open settings window: {error}");
            }
        }
    });
    Ok(())
}

pub(crate) fn show_settings_window(app: &AppHandle) -> tauri::Result<()> {
    show_settings_section(app, None)
}

#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SettingsSection {
    Appearance,
    Board,
    Agents,
    App,
}

impl SettingsSection {
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Board => "board",
            Self::Agents => "agents",
            Self::App => "app",
        }
    }
}

pub(crate) fn show_settings_section(
    app: &AppHandle,
    section: Option<SettingsSection>,
) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("settings") {
        if let Some(section) = section {
            // The section comes from a closed enum, never arbitrary script input.
            window.eval(format!(
                "window.location.hash = '/settings?section={}';",
                section.name()
            ))?;
        }
        window.show()?;
        return window.set_focus();
    }
    (|| -> tauri::Result<()> {
        let window = WebviewWindowBuilder::new(
            app,
            "settings",
            WebviewUrl::App(
                section
                    .map_or_else(
                        || "index.html#/settings".to_owned(),
                        |section| format!("index.html#/settings?section={}", section.name()),
                    )
                    .into(),
            ),
        )
        .title("Sideglance Settings")
        .inner_size(960.0, 720.0)
        .min_inner_size(720.0, 500.0)
        .resizable(true)
        // Keep settings and appearance events flowing while another app is focused.
        .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled)
        .center()
        .visible(false)
        .build()?;
        restore_native_frame(&window)?;
        window.show()?;
        window.set_focus()
    })()
}
