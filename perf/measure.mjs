// Measures the performance prototype (docs/PLAN.md, phase 0) through the DevTools protocol.
// Usage: `mise run bench`, which bundles the app and writes the benchmark session first.
// Needs Node.js 22 or later (global WebSocket). Prints a Markdown table.

import { execFileSync, spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const APP = "target/bundle/arbalette.app/Contents/MacOS/arbalette";
const PORT = 9333;
const SIDEBAR = "https://arbalette.ui/index.html";
const OVERLAY = `${SIDEBAR}#overlay`;
const RUNS = 20;

const dataDir = process.argv[2];
if (!dataDir) throw new Error("usage: node perf/measure.mjs <data dir>");

const started = performance.now();
const app = spawn(APP, [], {
  env: { ...process.env, ARBALETTE_DATA_DIR: dataDir, ARBALETTE_DEBUG_PORT: String(PORT) },
  stdio: "ignore",
});
try {
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(waitFor(`!!document.querySelector(".tabs .tab")`));
  const startup = performance.now() - started;
  const overlay = await connect(OVERLAY);

  const rows = [
    ["Cold start to a usable sidebar", [startup]],
    ["Switch tab, UI part (click → active tab painted)", await repeat(() => sidebar.eval(switchTab))],
    ["State update rendered (one title changed)", await repeat(() => sidebar.eval(stateUpdate))],
    ["Open the overlay (command → field focused, painted)", await repeat(() => openOverlay(sidebar, overlay))],
  ];
  console.log("| Measure | Median | p95 |\n|---|---|---|");
  for (const [name, times] of rows) {
    console.log(`| ${name} | ${ms(percentile(times, 0.5))} | ${ms(percentile(times, 0.95))} |`);
  }
  console.log(`\nJavaScript heap: sidebar ${await sidebar.heap()} MB, overlay ${await overlay.heap()} MB`);
  console.log(`Resident memory: ${memory()}`);
} finally {
  app.kill();
}

// Clicks a random tab of the first Space, waits for it to be shown as active, then for a frame.
async function switchTab() {
  const tabs = [...document.querySelectorAll(".tabs .tab:not([aria-current])")];
  const tab = tabs[Math.floor(Math.random() * tabs.length)];
  const start = performance.now();
  tab.click();
  await new Promise((resolve) => {
    new MutationObserver((_, observer) => {
      if (tab.getAttribute("aria-current")) resolve(observer.disconnect());
    }).observe(tab, { attributes: true });
  });
  await new Promise(requestAnimationFrame);
  return performance.now() - start;
}

// Replays the last state with one tab renamed, as the app does when a page changes its title.
async function stateUpdate() {
  const state = await new Promise((resolve) => {
    addEventListener("arbalette:state", (event) => resolve(event.detail), { once: true });
    fetch("/command/ready", { method: "POST" });
  });
  const title = `Renamed ${Math.random()}`;
  state.tabs[0] = { ...state.tabs[0], title };
  const start = performance.now();
  dispatchEvent(new CustomEvent("arbalette:state", { detail: state }));
  const tabs = document.querySelector(".tabs");
  await new Promise((resolve) => {
    new MutationObserver((_, observer) => {
      if (tabs.textContent.includes(title)) resolve(observer.disconnect());
    }).observe(tabs, { subtree: true, childList: true, characterData: true });
  });
  await new Promise(requestAnimationFrame);
  return performance.now() - start;
}

async function openOverlay(sidebar, overlay) {
  await overlay.eval(`(() => {
    window.shownAt = 0;
    addEventListener("arbalette:show", () => requestAnimationFrame(() => {
      if (document.activeElement?.tagName === "INPUT") shownAt = performance.timeOrigin + performance.now();
    }), { once: true });
  })()`);
  const start = await sidebar.eval(`(() => {
    fetch("/command/overlay/show", { method: "POST" });
    return performance.timeOrigin + performance.now();
  })()`);
  const shown = await overlay.eval(waitFor("window.shownAt"));
  await sidebar.eval(`fetch("/command/overlay/hide", { method: "POST" })`);
  await sleep(100);
  return shown - start;
}

// Connects to the page at `url` once the app has opened it.
async function connect(url) {
  let target;
  while (!target) {
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
  return {
    // Evaluates an expression, or calls a function, in the page and returns its value.
    async eval(code) {
      const expression = typeof code === "function" ? `(${code})()` : code;
      const params = { expression, awaitPromise: true, returnByValue: true };
      const result = await call("Runtime.evaluate", params);
      if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
      return result.result.value;
    },
    // The page's JavaScript heap in use, in MB.
    async heap() {
      const { usedSize } = await call("Runtime.getHeapUsage");
      return Math.round(usedSize / 1024 / 1024);
    },
  };
}

// An expression that resolves once `condition` is truthy, checked on every frame.
function waitFor(condition) {
  return `new Promise((resolve) => {
    const check = () => (${condition}) ? resolve(${condition}) : requestAnimationFrame(check);
    check();
  })`;
}

async function repeat(measure) {
  const times = [];
  for (let run = 0; run < RUNS; run++) times.push(await measure());
  return times;
}

function percentile(times, rank) {
  const sorted = [...times].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(rank * sorted.length))];
}

function ms(time) {
  return `${time.toFixed(1)} ms`;
}

// Resident memory of the app's processes, by process type ("browser" for the main one).
function memory() {
  const lines = execFileSync("ps", ["-A", "-o", "rss=,command="], { encoding: "utf8" }).split("\n");
  const types = new Map();
  for (const line of lines.filter((line) => line.includes("arbalette.app/Contents"))) {
    const type = line.match(/--type=([\w-]+)/)?.[1] ?? "browser";
    const megabytes = Number.parseInt(line, 10) / 1024;
    types.set(type, (types.get(type) ?? []).concat(Math.round(megabytes)));
  }
  return [...types].map(([type, sizes]) => `${type} ${sizes.join(" + ")} MB`).join(", ");
}
