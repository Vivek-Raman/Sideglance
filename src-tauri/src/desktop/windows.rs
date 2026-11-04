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
        let name = NSString::from_str(&format!("ctx-ppteer.{}", window.label()));
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
    app.on_menu_event(|app, event| match event.id().as_ref() {
        "settings" => show_settings_window(app),
        _ => {}
    });
    Ok(())
}

pub(crate) fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let result = (|| -> tauri::Result<()> {
        let window = WebviewWindowBuilder::new(
            app,
            "settings",
            WebviewUrl::App("index.html#/settings".into()),
        )
        .title("ctx-ppteer Settings")
        .inner_size(520.0, 430.0)
        .min_inner_size(420.0, 360.0)
        .resizable(true)
        .center()
        .visible(false)
        .build()?;
        restore_native_frame(&window)?;
        window.show()?;
        window.set_focus()
    })();
    if let Err(error) = result {
        eprintln!("Failed to open settings window: {error}");
    }
}
