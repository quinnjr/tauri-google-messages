// Zero-dependency tests for src/bridge.js using node:test + vm.
// The bridge is evaluated in a stubbed browser context (window, document,
// navigator, MutationObserver, __TAURI__) so its DOM/Tauri interactions
// can be asserted without a real webview.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const bridgeSrc = fs.readFileSync(path.join(here, "bridge.js"), "utf8");

function makeContext(initialTitle) {
  const invokes = [];
  const emits = [];
  let currentTitle = initialTitle;
  let observerCb = null;
  const listeners = {};

  function OrigNotification(title, opts) {
    this.title = title;
    this.opts = opts;
  }
  OrigNotification.prototype.show = function () {};
  OrigNotification.requestPermission = () => Promise.resolve("default");

  const sandbox = {
    console,
    navigator: { onLine: true },
    MutationObserver: class {
      constructor(cb) {
        observerCb = cb;
      }
      observe() {}
    },
    document: {
      get title() {
        return currentTitle;
      },
      querySelector: () => ({}),
      head: {},
    },
  };
  sandbox.window = {
    __TAURI__: {
      core: {
        invoke: (cmd, args) => {
          invokes.push({ cmd, args });
          return Promise.resolve();
        },
      },
      event: {
        emit: (event, payload) => {
          emits.push({ event, payload });
          return Promise.resolve();
        },
      },
    },
    Notification: OrigNotification,
    addEventListener: (type, cb) => {
      (listeners[type] ??= []).push(cb);
    },
  };
  sandbox.globalThis = sandbox;
  const ctx = vm.createContext(sandbox);
  return {
    invokes,
    emits,
    listeners,
    runBridge: () => vm.runInContext(bridgeSrc, ctx),
    fireObserver: () => observerCb?.(),
    setTitle: (t) => {
      currentTitle = t;
    },
    notify: (title, opts) =>
      vm.runInContext(
        `new window.Notification(${JSON.stringify(title)}, ${opts === undefined ? "undefined" : JSON.stringify(opts)})`,
        ctx,
      ),
    get NotificationFn() {
      return vm.runInContext("window.Notification", ctx);
    },
  };
}

test("title '(3) X' invokes set_unread with count 3", () => {
  const h = makeContext("(3) Inbox");
  h.runBridge();
  assert.equal(h.invokes.length, 1);
  assert.equal(h.invokes[0].cmd, "set_unread");
  assert.equal(h.invokes[0].args.count, 3);
});

test("title without prefix invokes set_unread with count 0", () => {
  const h = makeContext("Messages");
  h.runBridge();
  assert.equal(h.invokes.length, 1);
  assert.equal(h.invokes[0].cmd, "set_unread");
  assert.equal(h.invokes[0].args.count, 0);
});

test("title change re-reports the new count", () => {
  const h = makeContext("Messages");
  h.runBridge();
  h.setTitle("(7) Messages");
  h.fireObserver();
  assert.equal(h.invokes.at(-1).args.count, 7);
});

test("double eval wraps Notification once: one gm-notify per toast", () => {
  const h = makeContext("Messages");
  h.runBridge();
  h.runBridge();
  h.notify("Alice", { body: "hello" });
  const notifies = h.emits.filter((e) => e.event === "gm-notify");
  assert.equal(notifies.length, 1);
  assert.equal(notifies[0].payload.title, "Alice");
  assert.equal(notifies[0].payload.body, "hello");
});

test("missing body forwards empty string", () => {
  const h = makeContext("Messages");
  h.runBridge();
  h.notify("Ping");
  const notifies = h.emits.filter((e) => e.event === "gm-notify");
  assert.equal(notifies.length, 1);
  assert.equal(notifies[0].payload.title, "Ping");
  assert.equal(notifies[0].payload.body, "");
});

test("Notification keeps prototype chain and granted permission", async () => {
  const h = makeContext("Messages");
  const Orig = h.NotificationFn; // unwrapped constructor, before eval
  Orig.customMarker = "marker";
  h.runBridge();
  const N = h.NotificationFn;
  assert.equal(N.permission, "granted");
  assert.equal(await N.requestPermission(), "granted");
  assert.equal(N.prototype, Orig.prototype);
  assert.equal(Object.getPrototypeOf(N), Orig);
  assert.equal(N.customMarker, "marker");
  // Instance created through the wrapper is an instance of the original.
  const n = h.notify("T", { body: "b" });
  assert.ok(n instanceof Orig);
});
