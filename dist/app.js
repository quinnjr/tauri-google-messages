// Local splash loader (served from dist/ alongside index.html).
// The navigation target is a hardcoded constant below: there is no query
// parsing and nothing user-controlled influences the destination, so there
// is no open redirect.
window.__GM_TARGET = "https://messages.google.com/web";

function showOffline() {
  document.getElementById("loading").style.display = "none";
  document.getElementById("offline").style.display = "block";
}

function go() {
  if (!navigator.onLine) {
    showOffline();
    return;
  }
  // Preflight: online-but-unreachable (captive portal down, DNS failure)
  // would otherwise strand the user on a browser error page after
  // navigating. Only navigate when the target is reachable.
  fetch(window.__GM_TARGET, {
    mode: "no-cors",
    signal: AbortSignal.timeout(5000),
  }).then(
    () => {
      window.location.replace(window.__GM_TARGET);
    },
    showOffline,
  );
}

document.getElementById("retry").addEventListener("click", go);
window.addEventListener("online", go);
go();
