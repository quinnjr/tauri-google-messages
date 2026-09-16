use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

/// Set once the user picks Quit from the tray menu. The next
/// `CloseRequested` is then a real exit instead of a hide-to-tray.
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Show and focus the main window. Shared by the tray Show item, tray
/// left-click, and single-instance second-launch handling.
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn set_unread(app: tauri::AppHandle, count: u32) {
    let title = if count == 0 {
        "Google Messages".to_string()
    } else {
        format!("({count}) Google Messages")
    };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_title(&title);
    }
    // Tray tooltip reflects unread count; icon badge handled per-platform in Task 5 follow-up.
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&title));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![set_unread])
        .on_window_event(|window, event| {
            // Close-to-tray: hide the main window instead of closing it.
            // A real exit happens only via the tray Quit menu item.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && !QUIT_REQUESTED.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // ---- Task 4: tray icon + menu (BEGIN) ----
            // Task 5 appends the `gm-notify` event listener after this block.
            let show_i = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show_i, &quit_i]).build()?;

            let mut tray_builder = TrayIconBuilder::with_id("main")
                .tooltip("Google Messages")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main(app),
                    "quit" => {
                        QUIT_REQUESTED.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            let _tray = tray_builder.build(app)?;
            // ---- Task 4: tray icon + menu (END) ----

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
