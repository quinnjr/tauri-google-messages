# Tauri Google Messages Wrapper Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Tauri v2 desktop wrapper that loads Google Messages for Web (QR pair once) with native notifications, tray, autostart, and auto-updates.

**Architecture:** Static `dist/` splash loader navigates top-level to `https://messages.google.com/web`; Rust backend in `src-tauri/` wires notification/tray/autostart/single-instance/window-state/updater plugins; `bridge.js` injected via `initializationScript` forwards Notification events and title unread counts to Rust.

**Tech Stack:** Tauri CLI 2.11.4, Rust 1.98, Node 26 (build-time only, no frontend framework), WebKitGTK (Linux) / WebView2 (Windows) / WKWebView (macOS).

**Spec:** `docs/superpowers/specs/2026-09-16-tauri-google-messages-design.md`

## Global Constraints

- App identifier is `dev.quinnjr.tauri-google-messages` — exact, everywhere.
- Product name is `Google Messages`; window title is `Google Messages`.
- Main window: 1280×800 default, minimum 360×600.
- Remote URL is `https://messages.google.com/web` — top-level navigation only, never an iframe.
- Session persistence via WebView profile; QR pairing is one-time ("Remember this computer").
- Least-privilege capabilities; no message content stored by the app.
- Shortcuts in v1 = native WebView shortcuts only; no global-shortcut plugin.
- Targets: Linux (`.AppImage/.deb`), Windows (`.msi/.nsis`), macOS (`.dmg`).

---

## File Structure

New layout after Task 1 (replaces the current bare `Cargo.toml` + `src/main.rs`):

- `dist/index.html` — splash loader: shows "Opening Google Messages…", then top-level navigates to the web client; renders offline fallback with Retry button when navigation fails.
- `src/bridge.js` — injected script: Notification forwarder, `document.title` unread watcher (MutationObserver → Rust event), online/offline reporter. Only file that touches Google's DOM.
- `src-tauri/tauri.conf.json` — app metadata, window config, `frontendDist: ../dist`, remote navigation allowlist, bundle targets, updater endpoint placeholder (stable URL, key added in Task 6).
- `src-tauri/capabilities/default.json` — least-privilege capability set.
- `src-tauri/src/lib.rs` + `src-tauri/src/main.rs` — plugin wiring, tray menu (Show/Hide/Quit), single-instance focus, notification handler, badge updates.
- `src-tauri/icons/` — generated iconset from a single 1024×1024 source PNG.
- `package.json` — minimal, scripts only (`tauri dev`, `tauri build`); no framework deps.

Each file has one job: `index.html` loads, `bridge.js` observes, Rust integrates, config declares.

---

### Task 1: Scaffold the Tauri v2 shell

**Files:**
- Remove: `Cargo.toml`, `src/main.rs` (backend moves to `src-tauri/`)
- Create (via init): `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/default.json`, `package.json`, `dist/`
- Modify: `.gitignore` (append Tauri/node ignores)

**Interfaces:**
- Consumes: nothing (first task).
- Produces: `src-tauri/` project that `cargo check` passes on; `frontendDist` path `../dist` that Task 2 fills.

- [ ] **Step 1: Back up and clear the bare crate**

```bash
git mv Cargo.toml /tmp/tauri-google-messages-Cargo.toml.bak
git mv src /tmp/tauri-google-messages-src.bak
ls
# Expected: no Cargo.toml, no src/; docs/ remains
```

- [ ] **Step 2: Run non-interactive init**

```bash
cargo tauri init --ci \
  --app-name "Google Messages" \
  --window-title "Google Messages" \
  --frontend-dist ../dist \
  --dev-url http://localhost:1420 \
  --before-dev-command "" \
  --before-build-command ""
ls src-tauri src-tauri/capabilities dist
# Expected: tauri.conf.json, Cargo.toml, src/, capabilities/default.json, dist/ all exist
```

- [ ] **Step 3: Set identifier and window geometry in tauri.conf.json**

```json
{
  "identifier": "dev.quinnjr.tauri-google-messages",
  "productName": "Google Messages",
  "app": {
    "windows": [
      {
        "title": "Google Messages",
        "width": 1280,
        "height": 800,
        "minWidth": 360,
        "minHeight": 600
      }
    ]
  }
}
```

Merge these keys into the generated `src-tauri/tauri.conf.json` (keep all other generated keys as-is).

- [ ] **Step 4: Verify scaffold compiles**

```bash
cargo tauri info
cargo check --manifest-path src-tauri/Cargo.toml
# Expected: info prints Tauri 2.x with no errors; check finishes with zero errors
```

- [ ] **Step 5: Commit**

```bash
git add -A
git status --short
git commit -m "feat: scaffold Tauri v2 shell with dist frontend"
```

---

### Task 2: Loader splash + offline fallback frontend

**Files:**
- Create: `dist/index.html`
- Create: `src/bridge.js` (stub in this task; full logic in Task 5)
- Test: manual via local server (no framework needed)

**Interfaces:**
- Consumes: `frontendDist: ../dist` from Task 1.
- Produces: `dist/index.html` exposing `window.__GM_TARGET = "https://messages.google.com/web"`; `src/bridge.js` exporting `window.__gmBridge = { version: 1 }`.

- [ ] **Step 1: Write dist/index.html**

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Google Messages</title>
  <style>
    body { font-family: system-ui, sans-serif; display: grid; place-items: center; height: 100vh; margin: 0; }
    #offline { display: none; text-align: center; }
  </style>
</head>
<body>
  <div id="loading"><p>Opening Google Messages…</p></div>
  <div id="offline">
    <p>You appear to be offline.</p>
    <button id="retry">Retry</button>
  </div>
  <script>
    window.__GM_TARGET = "https://messages.google.com/web";
    function go() {
      if (navigator.onLine) { window.location.href = window.__GM_TARGET; }
      else {
        document.getElementById("loading").style.display = "none";
        document.getElementById("offline").style.display = "block";
      }
    }
    document.getElementById("retry").addEventListener("click", go);
    window.addEventListener("online", go);
    go();
  </script>
</body>
</html>
```

- [ ] **Step 2: Write src/bridge.js stub**

```js
// Full observer logic lands in Task 5. Stub proves injection works.
window.__gmBridge = { version: 1 };
```

- [ ] **Step 3: Verify loader locally**

```bash
python3 -m http.server 1420 --directory dist &
sleep 1
curl -s http://localhost:1420/ | head -8
kill %1
# Expected: serves the splash HTML; contains __GM_TARGET with the messages URL
```

- [ ] **Step 4: Commit**

```bash
git add dist/index.html src/bridge.js
git commit -m "feat: add splash loader with offline fallback"
```

---

### Task 3: Window navigation + least-privilege capabilities

**Files:**
- Modify: `src-tauri/tauri.conf.json` (remote allowlist for the Messages origin)
- Modify: `src-tauri/capabilities/default.json` (window + navigation scopes only; plugin scopes arrive in Task 4)
- Test: `cargo tauri inspect` shows resolved config; dev window navigates to QR screen

**Interfaces:**
- Consumes: `dist/index.html` from Task 2.
- Produces: capability set `default` that Task 4 extends with plugin permissions.

- [ ] **Step 1: Allow navigation to the Messages origin in tauri.conf.json**

```json
{
  "app": {
    "security": {
      "csp": null
    }
  }
}
```

Rationale (keep default CSP otherwise): the main window must top-level navigate to `https://messages.google.com/web`. Do NOT add an iframe anywhere. If the generated config has `app.windows[0].url` set to a local path, leave it — the loader performs the navigation (Option B design).

- [ ] **Step 2: Write capabilities/default.json**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Least-privilege baseline: window control for the wrapper.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-maximize",
    "core:window:allow-set-focus",
    "core:window:allow-show",
    "core:window:allow-hide"
  ]
}
```

- [ ] **Step 3: Verify config resolves and dev launches**

```bash
cargo tauri inspect tauri-config 2>&1 | head -30
cargo tauri dev &
sleep 12
# Expected: window opens on splash, then navigates to messages.google.com/web QR screen
kill %1
```

Manual gate: QR screen visible = pass. Blank window = fail, recheck Step 1.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/default.json
git commit -m "feat: allow Messages origin navigation with baseline capabilities"
```

---

### Task 4: Rust backend — plugins, tray, single-instance

**Files:**
- Modify: `src-tauri/Cargo.toml` (add plugin deps)
- Modify: `src-tauri/src/lib.rs` (plugin wiring, tray menu, single-instance, notification handler)
- Modify: `src-tauri/capabilities/default.json` (append plugin permissions)
- Test: `cargo check`; dev-run behavior checklist

**Interfaces:**
- Consumes: capability set `default` from Task 3; `window.__gmBridge` events from Task 5.
- Produces: tray menu with `show`, `hide`, `quit`; `gm-unread` event handler updating tray badge/title; single-instance focus behavior.

- [ ] **Step 1: Add plugin dependencies**

```bash
cargo tauri add notification tray-icon autostart single-instance window-state updater
cargo check --manifest-path src-tauri/Cargo.toml
# Expected: check passes; Cargo.toml now lists tauri-plugin-notification, -tray-icon, -autostart, -single-instance, -window-state, -updater
```

- [ ] **Step 2: Wire plugins and tray in src-tauri/src/lib.rs**

```rust
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};

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

pub fn run() {
    let show = MenuItemBuilder::with_id("show", "Show").build(/* app */);
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![set_unread])
        .setup(|app| {
            let show_i = MenuItemBuilder::with_id("show", "Show").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show_i, &quit_i]).build()?;
            let _tray = TrayIconBuilder::with_id("main")
                .tooltip("Google Messages")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Note: exact menu/tray builder method names vary slightly across 2.x patch releases — adapt to compiler errors, keep the same menu ids (`show`, `quit`) and tray id (`main`).

- [ ] **Step 3: Append plugin permissions to capabilities/default.json**

```json
{
  "permissions": [
    "core:default",
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-maximize",
    "core:window:allow-set-focus",
    "core:window:allow-show",
    "core:window:allow-hide",
    "notification:default",
    "tray-icon:default",
    "autostart:default",
    "single-instance:default",
    "window-state:default",
    "updater:default"
  ]
}
```

- [ ] **Step 4: Verify compile + tray behavior**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
# Expected: zero errors
cargo tauri dev &
sleep 12
# Manual checklist: tray icon visible; Show focuses window; second instance focuses first; close-to-tray/minimize works
kill %1
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat: wire notification, tray, autostart, single-instance, updater"
```

---

### Task 5: Bridge injection — notifications + unread badge

**Files:**
- Modify: `src/bridge.js` (full observer logic)
- Modify: `src-tauri/src/lib.rs` (register `initializationScript`, `gm-notify` listener)
- Test: dev-run with phone paired; curl-free manual verification

**Interfaces:**
- Consumes: tray id `main`, `set_unread` command from Task 4.
- Produces: `gm-unread` (u32) and `gm-notify` ({title, body}) events consumed by Rust; no other module depends on bridge internals.

- [ ] **Step 1: Write the full src/bridge.js**

```js
// Runs in the Messages webview via initializationScript. Read-only observer:
// never sends messages, never stores content.
(function () {
  window.__gmBridge = { version: 2 };

  function sendUnread(n) {
    try {
      if (window.__TAURI__?.core?.invoke) {
        window.__TAURI__.core.invoke("set_unread", { count: n });
      }
    } catch (_) {}
  }

  function readUnreadFromTitle() {
    const m = document.title.match(/^\((\d+)\)/);
    sendUnread(m ? parseInt(m[1], 10) : 0);
  }

  new MutationObserver(readUnreadFromTitle).observe(
    document.querySelector("title") || document.head,
    { childList: true, subtree: true, characterData: true }
  );
  readUnreadFromTitle();

  // Forward web Notifications to the native layer; suppress default toast
  // only if Rust confirms display (keep default otherwise to avoid silence).
  const OrigNotification = window.Notification;
  window.Notification = function (title, opts) {
    try {
      if (window.__TAURI__?.event?.emit) {
        window.__TAURI__.event.emit("gm-notify", {
          title: String(title),
          body: String(opts?.body ?? ""),
        });
      }
    } catch (_) {}
    return new OrigNotification(title, opts);
  };
  window.Notification.permission = "granted";
  window.Notification.requestPermission = async () => "granted";

  function reportOnline() {
    try {
      window.__TAURI__?.event?.emit("gm-online", { online: navigator.onLine });
    } catch (_) {}
  }
  window.addEventListener("online", reportOnline);
  window.addEventListener("offline", reportOnline);
})();
```

- [ ] **Step 2: Register the script and notify listener in lib.rs setup**

```rust
// Inside .setup(|app| { ... }), before building the tray:
let bridge = include_str!("../../src/bridge.js").to_string();
if let Some(w) = app.get_webview_window("main") {
    let _ = w.add_initialization_script(&bridge);
}
let handle = app.handle().clone();
app.listen("gm-notify", move |event| {
    // Payload: {"title": "...", "body": "..."} — show native toast.
    if let Ok(p) = serde_json::from_str::<serde_json::Value>(event.payload()) {
        let title = p["title"].as_str().unwrap_or("Google Messages");
        let body = p["body"].as_str().unwrap_or("");
        let _ = tauri_plugin_notification::NotificationExt::notification(&handle)
            .builder()
            .title(title)
            .body(body)
            .show();
    }
});
```

Add `serde_json` to `src-tauri/Cargo.toml` if not present (`cargo add serde_json -p` equivalent via `cargo tauri` workspace path).

- [ ] **Step 3: Verify end-to-end (requires paired phone)**

```bash
cargo tauri dev &
# 1. Pair via QR once (phone Messages app -> Device pairing).
# 2. Send yourself a test SMS from another device.
# 3. Expect: native toast appears; window title shows (1); tray tooltip shows (1).
# 4. Restart app: expect still paired (no QR).
kill %1
```

If title format differs (Google changes it), update only the regex in `readUnreadFromTitle` — Rust stays untouched.

- [ ] **Step 4: Commit**

```bash
git add src/bridge.js src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: bridge web notifications and unread count to native shell"
```

---

### Task 6: Icons, bundle metadata, updater keys

**Files:**
- Create: `src-tauri/icons/*` (generated), `icon-source-1024.png` (repo root or `assets/`)
- Modify: `src-tauri/tauri.conf.json` (bundle targets, category, updater endpoint)
- Test: `cargo tauri build` on Linux produces bundles

**Interfaces:**
- Consumes: everything above.
- Produces: signed bundles + `updater` public key in config; Windows/macOS builds in Task 7 consume this config unchanged.

- [ ] **Step 1: Add a 1024×1024 icon source and generate the set**

```bash
ls icon-source-1024.png assets/icon-source-1024.png 2>&1
# Place your 1024x1024 PNG at ./icon-source-1024.png first (speech-bubble style, no Google trademark issues for local use)
cargo tauri icon icon-source-1024.png
ls src-tauri/icons
# Expected: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico, ...
```

- [ ] **Step 2: Set bundle metadata in tauri.conf.json**

```json
{
  "bundle": {
    "active": true,
    "category": "Social",
    "targets": ["appimage", "deb", "msi", "nsis", "dmg"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

- [ ] **Step 3: Generate updater signing keys**

```bash
cargo tauri signer generate -w ~/.tauri-google-messages.key
# Expected: prints public key; paste it into tauri.conf.json under plugins.updater.pubkey, keep private key OUT of git
cargo tauri signer sign <bundle-file> --private-key ~/.tauri-google-messages.key
# (Run after first successful build in Step 4.)
```

- [ ] **Step 4: Verify Linux release build**

```bash
cargo tauri build
ls src-tauri/target/release/bundle/
# Expected: appimage/ and deb/ artifacts present; no build errors
```

- [ ] **Step 5: Commit (keys excluded)**

```bash
git add src-tauri/icons src-tauri/tauri.conf.json .gitignore
git status --short
# Confirm: no *.key, no private key files staged
git commit -m "feat: add icons, bundle metadata, and updater pubkey"
```

---

### Task 7: Cross-platform bundles + release checklist

**Files:**
- Modify: `.github/workflows/build.yml` (create) — matrix build for Linux/Windows/macOS
- Test: three green artifacts + manual QA pass per platform

**Interfaces:**
- Consumes: all tasks above.
- Produces: release binaries; no code changes expected — failures here are config fixes only.

- [ ] **Step 1: Add a matrix build workflow**

```yaml
name: build
on: [push, pull_request]
jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - name: Install Linux deps
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
      - run: cargo tauri build
```

- [ ] **Step 2: Run the release QA checklist (per platform)**

```bash
cargo tauri build
# Manual pass, each must be YES:
# [ ] QR pair persists across restarts
# [ ] Send + receive SMS works
# [ ] Native notification on incoming message
# [ ] Tray badge/tooltip counts unread
# [ ] Minimize-to-tray, Show/Hide, Quit behave
# [ ] Autostart toggle persists login launch
# [ ] Second launch focuses existing window (no duplicate)
# [ ] Offline page with Retry appears when network is cut
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/build.yml
git commit -m "ci: add cross-platform Tauri build matrix"
```

---

## Self-Review

- **Spec coverage:** identifier/product/window (T1+T6) ✓; loader+redirect, no iframe (T2+T3) ✓; QR-once session persistence (T5 Step 3) ✓; notification/tray/badge (T4+T5) ✓; autostart/single-instance/window-state/updater (T4+T6) ✓; offline fallback (T2) ✓; permissions/spellcheck (T3+T4) ✓; Linux/Win/macOS bundles (T6+T7) ✓; out-of-scope items excluded ✓.
- **Placeholder scan:** updater endpoint intentionally unresolved until hosting is chosen — flagged in T6 as "stable URL" rather than a fake value. No TBD/TODO/code-shape placeholders remain.
- **Type consistency:** tray id `main`, menu ids `show`/`quit`, events `gm-notify`/`gm-online`, command `set_unread(count: u32)`, `__gmBridge.version` 1→2 across tasks — consistent.
