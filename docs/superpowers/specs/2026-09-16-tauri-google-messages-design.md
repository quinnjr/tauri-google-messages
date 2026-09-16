# Tauri Google Messages Wrapper — Design Spec
Date: 2026-09-16
Status: Approved (all 4 sections)
Approach: Option B — local loader + JS bridge (approved)

## 1. Goal
A Tauri v2 desktop app that loads the user's phone text messages and sends
via the Google Messages system, by wrapping `https://messages.google.com/web`
(pair via QR, like WhatsApp Web). No public Google Messages API exists, so the
app is a thin, polished native shell — not a protocol reimplementation.

## 2. Architecture & Components
- **Stack:** Tauri v2 (Rust backend + static minimal frontend). Frontend is a
  tiny `dist/index.html` splash/loader + `src/bridge.js`; no framework. Node
  is build-time only.
- **Identifier:** `dev.quinnjr.tauri-google-messages` (user-supplied).
  Product name: "Google Messages".
- **Layout (post-scaffold):**
  - `src-tauri/` — Rust backend, `tauri.conf.json`, `capabilities/`, icons.
  - `dist/index.html` — splash → top-level navigation to Messages for Web.
  - `src/bridge.js` — injected via `initializationScript`.
- **Main window:** 1280×800, min 360×600. Loads local splash first, then
  navigates top-level to `https://messages.google.com/web`. No iframe
  (Google sends X-Frame-Options DENY).
- **Rust plugins (v2-compatible):** `notification` (native toasts),
  `tray-icon` (tray + unread badge + show/hide + quit), `autostart` (login
  launch), `  single-instance` (focus existing), `window-state` (remember
  geometry), `updater` (signed auto-updates). Shortcuts in v1 = native
  WebView shortcuts only (copy/paste/find/reload); no global-shortcut
  plugin.
- **Capabilities:** least privilege — `core:window:allow-*` subset,
  notification/tray/autostart scopes only.
- **JS bridge duties:** forward web `Notification` to Rust, watch
  `document.title` `(N)` unread via MutationObserver → tray badge, report
  online/offline for fallback page.

## 3. Data Flow & Session
1. First launch → splash → redirect to Messages for Web → QR screen.
2. User pairs: phone Messages app → Device pairing → scan QR.
3. Google links browser session to phone; SMS/RCS relay through Google's
   servers. App never touches the phone directly, stores no message content.
4. WebView profile cookies/localStorage persist pairing ("Remember this
   computer"); later launches skip QR.
5. Send/receive happens inside Google's client; bridge only observes
   title/notifications for tray + toasts.
6. Unpair/logout from either side → QR screen returns; only WebView profile
   data to clear.

## 4. Error Handling, Permissions & Platforms
- **Offline/load failure:** local fallback page with retry
  (`navigator.onLine` + load-error listener); never a blank window.
- **Session expiry:** QR screen reappears naturally; no custom recovery.
- **Permissions:** notifications allowed; camera/mic/file-access granted for
  RCS attachments/voice via capability scopes; spellcheck via WebView
  default + context menu.
- **Platforms (one codebase):** Linux (WebKitGTK, `~/.config`), Windows
  (WebView2), macOS (WKWebView). Bundles: `.AppImage/.deb`, `.msi/.nsis`,
  `.dmg`.
- **Single-instance + window-state:** no duplicates; geometry remembered.
- **Updater:** Tauri signed bundles with private/public key pair.

## 5. Testing, Success Criteria & Scope
- **Out of scope (v1):** message backup/export, multi-account, E2E handling
  (Google's), custom themes. Wrapper stays thin.
- **Done =** `cargo check` + `tauri build` green on Linux; manual pass:
  QR pair persists across restarts, send/receive works, native notification
  on incoming, tray badge counts unread, minimize-to-tray + autostart +
  single-instance behave, offline page + retry works, bundles for all 3 OSes.
- **Maintenance:** Google web-client changes to title/Notification shape
  touch only `bridge.js`; Rust stays stable.

## 6. Decisions Log
- Wrapper around Messages for Web (not native SMS sync) — confirmed.
- Platforms: Linux + Windows + macOS.
- Features: Full polish (persistent session, icon, notifications, tray,
  minimize-to-tray, badge, autostart, spellcheck, media perms, shortcuts,
  auto-updates).
- Tauri v2 minimal static frontend; replace bare Cargo crate — confirmed.
- Identifier: `dev.quinnjr.tauri-google-messages` — user-supplied.
