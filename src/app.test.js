// Zero-dependency tests for dist/app.js using node:test + vm.
// The loader is evaluated in a stubbed browser context (window, document,
// navigator, fetch, AbortSignal) so navigation vs. offline-fallback
// behavior can be asserted without a real webview.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const appSrc = fs.readFileSync(path.join(here, "..", "dist", "app.js"), "utf8");

const TARGET = "https://messages.google.com/web";

function makeContext({ online, fetchImpl }) {
  const replaced = [];
  const listeners = {};
  const fetchCalls = [];
  const elements = {
    loading: { style: {} },
    offline: { style: {} },
    retry: {
      addEventListener: (type, cb) => {
        (listeners[`retry:${type}`] ??= []).push(cb);
      },
    },
  };

  const sandbox = {
    console,
    navigator: { onLine: online },
    AbortSignal,
    fetch: (url, opts) => {
      fetchCalls.push({ url, opts });
      return fetchImpl(url, opts);
    },
    document: {
      getElementById: (id) => elements[id],
    },
  };
  sandbox.window = {
    __GM_TARGET: undefined,
    location: {
      replace: (url) => {
        replaced.push(url);
      },
    },
    addEventListener: (type, cb) => {
      (listeners[type] ??= []).push(cb);
    },
  };
  sandbox.globalThis = sandbox;
  const ctx = vm.createContext(sandbox);
  return {
    replaced,
    listeners,
    fetchCalls,
    elements,
    runApp: () => vm.runInContext(appSrc, ctx),
    target: () => vm.runInContext("window.__GM_TARGET", ctx),
  };
}

const flush = () => new Promise((resolve) => setImmediate(resolve));

test("target stays the hardcoded constant (no query parsing / open redirect)", () => {
  const h = makeContext({ online: true, fetchImpl: () => Promise.resolve({}) });
  h.runApp();
  assert.equal(h.target(), TARGET);
  assert.ok(!appSrc.includes("location.search"));
  assert.ok(!appSrc.includes("URLSearchParams"));
});

test("fetch success navigates to the target with replace", async () => {
  const h = makeContext({ online: true, fetchImpl: () => Promise.resolve({}) });
  h.runApp();
  await flush();
  await flush();
  assert.deepEqual(h.replaced, [TARGET]);
  assert.notEqual(h.elements.offline.style.display, "block");
});

test("fetch failure shows offline UI and never navigates", async () => {
  const h = makeContext({
    online: true,
    fetchImpl: () => Promise.reject(new Error("dns")),
  });
  h.runApp();
  await flush();
  await flush();
  assert.deepEqual(h.replaced, []);
  assert.equal(h.elements.loading.style.display, "none");
  assert.equal(h.elements.offline.style.display, "block");
});

test("offline at load shows offline UI without fetching", async () => {
  let fetched = false;
  const h = makeContext({
    online: false,
    fetchImpl: () => {
      fetched = true;
      return Promise.resolve({});
    },
  });
  h.runApp();
  await flush();
  assert.equal(fetched, false);
  assert.deepEqual(h.replaced, []);
  assert.equal(h.elements.loading.style.display, "none");
  assert.equal(h.elements.offline.style.display, "block");
});
