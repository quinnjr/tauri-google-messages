// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's dmabuf renderer crashes on several Wayland compositors with
    // "Error 71 (Protocol error) dispatching to Wayland display" at startup.
    // Disable it unless the user explicitly opted in/out. Must run before any
    // WebKit/GTK initialization, hence here and not in setup().
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: single-threaded at process entry; no other thread exists yet.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    app_lib::run();
}
