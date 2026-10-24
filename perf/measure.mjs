// Measures the performance prototype (docs/PLAN.md, phase 0) through the DevTools protocol.
// Usage: `mise run bench`, which bundles the app and writes the benchmark session first.
// Prints a Markdown table.

import { execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { connect, launch, OVERLAY, SIDEBAR, waitFor } from "../e2e/devtools.mjs";

const RUNS = 20;

const dataDir = process.argv[2];
if (!dataDir) throw new Error("usage: node perf/measure.mjs <data dir>");

// macOS checks a new build's code on its first launch, which takes over a second: measured apart.
const firstLaunch = await start();
firstLaunch.app.kill();
await new Promise((resolve) => firstLaunch.app.once("exit", resolve));
const { app, sidebar, startup } = await start();
try {
  const overlay = await connect(OVERLAY);

  const rows = [
    ["First launch of a new build to a usable sidebar", [firstLaunch.startup]],
    ["Cold start to a usable sidebar", [startup]],
    ["Switch tab, UI part (click → active tab painted)", await repeat(() => sidebar.eval(switchTab))],
    ["State update rendered (one title changed)", await repeat(() => sidebar.eval(stateUpdate))],
    ["Menu targets sent (one folder fewer → app told)", await repeat(() => sidebar.eval(menuTargetsUpdate))],
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

// Launches the app and waits until its sidebar shows tabs.
async function start() {
  const started = performance.now();
  const app = launch(dataDir);
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(waitFor(`!!document.querySelector(".tabs .tab")`));
  await sidebar.eval(`window.shown = () => document.querySelector(".space-page:not([inert])")`);
  return { app, sidebar, startup: performance.now() - started };
}

// Clicks a random tab of the first Space, waits for it to be shown as active, then for a frame.
async function switchTab() {
  const tabs = [...shown().querySelectorAll(".tabs .tab:not([aria-current])")];
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

// Replays the last state with one tab renamed, as the app does when a page changes its title, and
// times the update until the sidebar is laid out again.
async function stateUpdate() {
  const state = await new Promise((resolve) => {
    addEventListener("arbalette:state", (event) => resolve(event.detail), { once: true });
    fetch("/command/ready", { method: "POST" });
  });
  const title = `Renamed ${Math.random()}`;
  const page = state.pages.find((page) => page.space === state.space);
  page.today[0] = { ...page.today[0], title };
  const start = performance.now();
  dispatchEvent(new CustomEvent("arbalette:state", { detail: state }));
  const tabs = shown().querySelector(".today");
  // React may render during the dispatch, before an observer could see the change.
  if (!tabs.textContent.includes(title)) {
    await new Promise((resolve) => {
      new MutationObserver((_, observer) => {
        if (tabs.textContent.includes(title)) resolve(observer.disconnect());
      }).observe(tabs, { subtree: true, childList: true, characterData: true });
    });
  }
  // Forces style and layout; waiting for the next frame would measure the display's refresh rate.
  tabs.getBoundingClientRect();
  return performance.now() - start;
}

// Replays the last state without the first pinned folder, which moves the rows below it, and times
// the update until the sidebar tells the app where its rows now are (ui/src/menuTargets.ts).
async function menuTargetsUpdate() {
  const state = await new Promise((resolve) => {
    addEventListener("arbalette:state", (event) => resolve(event.detail), { once: true });
    fetch("/command/ready", { method: "POST" });
  });
  // The state brings the folder back: its targets go first.
  for (let i = 0; i < 3; i++) await new Promise(requestAnimationFrame);
  // A new state, as the sidebar already shows this one.
  const update = structuredClone(state);
  const page = update.pages.find((page) => page.space === update.space);
  page.pinned = page.pinned.slice(1);
  const fetchCommand = window.fetch;
  const sent = new Promise((resolve) => {
    window.fetch = (url, init) => {
      if (String(url).includes("/command/menu/targets/")) {
        window.fetch = fetchCommand;
        resolve();
      }
      return fetchCommand(url, init);
    };
  });
  const start = performance.now();
  dispatchEvent(new CustomEvent("arbalette:state", { detail: update }));
  await sent;
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
