// Runs the bundled app and drives its UI pages through the DevTools protocol.
// Shared by the end-to-end checks (e2e/tabs.mjs, e2e/import.mjs) and the benchmark (perf/measure.mjs).
// Needs Node.js 22 or later (global WebSocket).

import { spawn } from "node:child_process";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const APP = resolve("target/bundle/arbalette.app/Contents/MacOS/arbalette");
const PORT = 9333;
const TIMEOUT = 10_000;

export const SIDEBAR = "https://arbalette.ui/index.html";
export const OVERLAY = `${SIDEBAR}#overlay`;
export const REVEAL = `${SIDEBAR}#reveal`;
export const IMPORT = `${SIDEBAR}#import`;

// Starts the app on the session in `dataDir`, offering to import the browsers found in `home` on
// a first launch; the caller kills it. By default a home without browsers, never the machine's.
export function launch(dataDir, home = join(tmpdir(), "arbalette-no-home")) {
  return spawn(APP, [], {
    env: {
      ...process.env,
      ARBALETTE_DATA_DIR: dataDir,
      ARBALETTE_HOME: home,
      ARBALETTE_DEBUG_PORT: String(PORT),
    },
    stdio: "ignore",
    // Not the repository, which may sit in Documents: macOS asks before an app reads it.
    cwd: tmpdir(),
  });
}

// The urls of the pages open in the app.
export async function pages() {
  const list = await fetch(`http://127.0.0.1:${PORT}/json/list`).then((response) => response.json());
  return list.map((page) => page.url);
}

// Connects to the page at `url` once the app has opened it.
export async function connect(url) {
  const deadline = performance.now() + TIMEOUT;
  let target;
  while (!target) {
    if (performance.now() > deadline) throw new Error(`no page at ${url}`);
    const list = await fetch(`http://127.0.0.1:${PORT}/json/list`).then(
      (response) => response.json(),
      () => [],
    );
    target = list.find((page) => page.url === url);
    if (!target) await sleep(20);
  }
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve) => socket.addEventListener("open", resolve, { once: true }));
  const pending = new Map();
  let id = 0;
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    pending.get(message.id)?.(message);
    pending.delete(message.id);
  });
  const call = (method, params) =>
    new Promise((resolve, reject) => {
      pending.set(++id, (message) =>
        message.error ? reject(new Error(message.error.message)) : resolve(message.result),
      );
      socket.send(JSON.stringify({ id, method, params }));
    });
  // Evaluates an expression, or calls a function, in the page and returns its value.
  const evaluate = async (code) => {
    const expression = typeof code === "function" ? `(${code})()` : code;
    const params = { expression, awaitPromise: true, returnByValue: true };
    const result = await call("Runtime.evaluate", params);
    if (result.exceptionDetails) {
      const { exception, text } = result.exceptionDetails;
      throw new Error(exception?.description ?? text);
    }
    return result.result.value;
  };
  // The page is listed before its document replaces the initial empty one.
  const loaded = `location.href === ${JSON.stringify(url)} && document.readyState === "complete"`;
  while (!(await evaluate(loaded).catch(() => false))) {
    if (performance.now() > deadline) throw new Error(`${url} did not load`);
    await sleep(20);
  }
  return {
    eval: evaluate,
    // Calls a DevTools protocol method on the page.
    call,
    // The page's JavaScript heap in use, in MB.
    async heap() {
      const { usedSize } = await call("Runtime.getHeapUsage");
      return Math.round(usedSize / 1024 / 1024);
    },
  };
}

// An expression that resolves once `condition` is truthy, checked on every frame, and rejects
// after a timeout. A hidden page draws no frame: it is then checked on a timer.
export function waitFor(condition) {
  return `new Promise((resolve, reject) => {
    const deadline = performance.now() + ${TIMEOUT};
    const check = () => {
      if (${condition}) return resolve(${condition});
      if (performance.now() > deadline) return reject(new Error(${JSON.stringify(`timed out waiting for ${condition}`)}));
      let checked = false;
      const next = () => checked || ((checked = true), check());
      requestAnimationFrame(next);
      setTimeout(next, 50);
    };
    check();
  })`;
}
