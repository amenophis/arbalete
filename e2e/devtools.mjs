// Runs the bundled app and drives its UI pages through the DevTools protocol.
// Shared by the end-to-end check (e2e/tabs.mjs) and the benchmark (perf/measure.mjs).
// Needs Node.js 22 or later (global WebSocket).

import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const APP = "target/bundle/arbalette.app/Contents/MacOS/arbalette";
const PORT = 9333;
const TIMEOUT = 10_000;

export const SIDEBAR = "https://arbalette.ui/index.html";
export const OVERLAY = `${SIDEBAR}#overlay`;

// Starts the app on the session in `dataDir`; the caller kills it.
export function launch(dataDir) {
  return spawn(APP, [], {
    env: { ...process.env, ARBALETTE_DATA_DIR: dataDir, ARBALETTE_DEBUG_PORT: String(PORT) },
    stdio: "ignore",
  });
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
    // The page's JavaScript heap in use, in MB.
    async heap() {
      const { usedSize } = await call("Runtime.getHeapUsage");
      return Math.round(usedSize / 1024 / 1024);
    },
  };
}

// An expression that resolves once `condition` is truthy, checked on every frame, and rejects
// after a timeout.
export function waitFor(condition) {
  return `new Promise((resolve, reject) => {
    const deadline = performance.now() + ${TIMEOUT};
    const check = () => {
      if (${condition}) return resolve(${condition});
      if (performance.now() > deadline) return reject(new Error(${JSON.stringify(`timed out waiting for ${condition}`)}));
      requestAnimationFrame(check);
    };
    check();
  })`;
}
