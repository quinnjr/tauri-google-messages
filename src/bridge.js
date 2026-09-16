// Runs in the Messages webview via on_page_load eval. Read-only observer:
// never sends messages, never stores content.
(function () {
  // Idempotency guard: eval-based injection re-runs per page load, and must
  // never double-wrap Notification (that would double-emit gm-notify).
  if (window.__gmBridge && window.__gmBridge.version >= 2) return;
  window.__gmBridge = { version: 2 };

  function sendUnread(n) {
    try {
      if (window.__TAURI__?.core?.invoke) {
        window.__TAURI__.core.invoke("set_unread", { count: n });
      }
    } catch (e) {
      console.warn("[gm-bridge] set_unread failed", e);
    }
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
  window.Notification.prototype = OrigNotification.prototype;
  Object.setPrototypeOf(window.Notification, OrigNotification);
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
