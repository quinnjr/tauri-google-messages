use std::sync::atomic::{AtomicBool, Ordering};

use tauri::Listener;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

/// Set once the user picks Quit from the tray menu. The next
/// `CloseRequested` is then a real exit instead of a hide-to-tray.
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

const APP_TITLE: &str = "Google Messages";

fn title_for(count: u32) -> String {
    if count == 0 {
        APP_TITLE.to_string()
    } else {
        format!("({count}) {APP_TITLE}")
    }
}

/// Show and focus the main window. Shared by the tray Show item, tray
/// left-click, and single-instance second-launch handling.
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if let Err(e) = w.unminimize() {
            log::error!("failed to unminimize main window: {e}");
        }
        if let Err(e) = w.show() {
            log::error!("failed to show main window: {e}");
        }
        if let Err(e) = w.set_focus() {
            log::error!("failed to focus main window: {e}");
        }
    }
}

/// Check for updates from the configured GitHub Releases endpoint,
/// download + install when one is available, then restart to apply it.
/// All failures are logged; the check never panics and never touches
/// frontend code (updater capability permissions intentionally absent).
async fn check_for_updates(app: tauri::AppHandle) {
    use tauri_plugin_updater::UpdaterExt;

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            log::error!("failed to get updater: {e}");
            return;
        }
    };
    let update = match updater.check().await {
        Ok(u) => u,
        Err(e) => {
            log::error!("update check failed: {e}");
            return;
        }
    };
    let Some(update) = update else {
        log::debug!("up to date");
        return;
    };
    log::info!("update available: {}", update.version);
    if let Err(e) = update
        .download_and_install(
            |chunk_len, content_len| match content_len {
                Some(total) => log::info!("downloaded {chunk_len} bytes of {total}"),
                None => log::info!("downloaded {chunk_len} bytes"),
            },
            || {
                log::info!("download finished, installing update");
            },
        )
        .await
    {
        log::error!("failed to download and install update: {e}");
        return;
    }
    // `AppHandle::restart` is core Tauri (no new plugin dependency):
    // on Windows the NSIS/MSI installer relaunches via
    // `restart_after_install` anyway; on macOS/Linux a relaunch is
    // required to run the newly installed version.
    log::info!("update installed; restarting");
    app.restart();
}

/// Treat notification content as untrusted: strip control characters and
/// cap length before building the native toast.
fn sanitize_notify_text(s: &str, max_chars: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

#[tauri::command]
fn set_unread(app: tauri::AppHandle, count: i64) {
    // Invoke contract: JS sends a JSON number via `invoke("set_unread",
    // { count })`; serde deserializes it to i64 here, then we clamp.
    let count = count.clamp(0, 9999) as u32;
    let title = title_for(count);
    if let Some(w) = app.get_webview_window("main") {
        if let Err(e) = w.set_title(&title) {
            log::error!("failed to set window title: {e}");
        }
    }
    // TODO: per-platform tray icon badge (tooltip/title carry the count until then).
    if let Some(tray) = app.tray_by_id("main") {
        if let Err(e) = tray.set_tooltip(Some(&title)) {
            log::error!("failed to set tray tooltip: {e}");
        }
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
                    if let Err(e) = window.hide() {
                        log::error!("failed to hide main window: {e}");
                    }
                }
            }
        })
        .on_page_load(|webview, payload| {
            // Init-script equivalent: re-inject the read-only observer after
            // every full page load (fresh JS context per navigation). The
            // bridge guards against double-install in the same context.
            //
            // Accepted gap: there is no document-start injection path for
            // the config-declared `main` window in Tauri 2.11 —
            // `initialization_script` exists only on the
            // `WebviewBuilder`/`WebviewWindowBuilder` builders (verified
            // against the vendored 2.11.5 sources; `WindowConfig` has no
            // init-script field), and `PageLoadEvent` offers only
            // `Started`/`Finished`. A Notification constructed between page
            // boot and Finished therefore uses the native constructor (a
            // web toast, never forwarded); unread state is still caught by
            // the install-time title re-read, so only first-seconds toasts
            // fall back to web rendering. Closing the gap would require
            // creating the main window programmatically via
            // `WebviewWindowBuilder::initialization_script`, i.e. giving up
            // the config-declared window — not worth the restructuring.
            if webview.label() == "main"
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                let bridge = include_str!("../../src/bridge.js");
                if let Err(e) = webview.eval(bridge) {
                    log::error!("failed to inject gm bridge: {e}");
                }
            }
        })
        .setup(|app| {
            // Release logging: stdout + log-dir file + webview console, in
            // all builds (a debug-only plugin would drop every log call in
            // release). Rotation/file-open behavior is the plugin default.
            app.handle().plugin(
                tauri_plugin_log::Builder::new()
                    .targets([
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                            file_name: Some("google-messages".into()),
                        }),
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                    ])
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // NOTE: the brief's `add_initialization_script` does not
            // exist on `WebviewWindow` in Tauri 2 — `initialization_script`
            // is a builder-only API (`WebviewBuilder` /
            // `WebviewWindowBuilder`), with no `WindowConfig` equivalent
            // for config-declared windows. Injection happens in
            // `.on_page_load` above (eval on Finished for the main
            // webview) instead; see the accepted-gap note there.

            let show_i = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let check_updates_i =
                MenuItemBuilder::with_id("check-updates", "Check for Updates").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&show_i, &check_updates_i, &quit_i])
                .build()?;

            let mut tray_builder = TrayIconBuilder::with_id("main")
                .tooltip(APP_TITLE)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main(app),
                    "check-updates" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            check_for_updates(handle).await;
                        });
                    }
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
            } else {
                log::debug!("no default window icon; tray built without an icon");
            }
            let _tray = tray_builder.build(app)?;

            let handle = app.handle().clone();
            app.listen("gm-notify", move |event| {
                // Payload: {"title": "...", "body": "..."} — show native toast.
                match serde_json::from_str::<serde_json::Value>(event.payload()) {
                    Ok(p) => {
                        let title = p["title"].as_str().unwrap_or(APP_TITLE);
                        let body = p["body"].as_str().unwrap_or("");
                        let title = sanitize_notify_text(title, 100);
                        let body = sanitize_notify_text(body, 200);
                        let title = if title.is_empty() {
                            APP_TITLE.to_string()
                        } else {
                            title
                        };
                        if let Err(e) =
                            tauri_plugin_notification::NotificationExt::notification(&handle)
                                .builder()
                                .title(title)
                                .body(body)
                                .show()
                        {
                            log::error!("failed to show notification: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("ignoring malformed gm-notify payload: {e}");
                    }
                }
            });

            let _online_id = app.listen("gm-online", |event| {
                log::debug!("gm-online: {}", event.payload());
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_count_yields_plain_title() {
        assert_eq!(title_for(0), "Google Messages");
    }

    #[test]
    fn one_count_is_prefixed() {
        assert_eq!(title_for(1), "(1) Google Messages");
    }

    #[test]
    fn large_count_is_prefixed() {
        assert_eq!(title_for(42), "(42) Google Messages");
    }

    #[test]
    fn sanitize_strips_control_characters() {
        assert_eq!(sanitize_notify_text("a\x00b\x07c\u{7f}d", 100), "abcd");
    }

    #[test]
    fn sanitize_caps_length() {
        assert_eq!(sanitize_notify_text("abcdef", 3), "abc");
    }

    #[test]
    fn sanitize_strips_newlines_too() {
        assert_eq!(sanitize_notify_text("Hi\nthere", 200), "Hithere");
    }
}
