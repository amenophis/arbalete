// Measures the performance prototype (docs/PLAN.md, phase 0) through the DevTools protocol.
// Usage: `mise run bench`, which bundles the app and writes the benchmark session first.
// Prints a Markdown table.

import { execFileSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { connect, launch, OVERLAY, SIDEBAR, waitFor } from "../e2e/devtools.mjs";

const RUNS = 20;

const dataDir = process.argv[2];
if (!dataDir) throw new Error("usage: node perf/measure.mjs <data dir>");

const started = performance.now();
const app = launch(dataDir);
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

// Replays the last state with one tab renamed, as the app does when a page changes its title, and
// times the update until the sidebar is laid out again.
async function stateUpdate() {
  const state = await new Promise((resolve) => {
    addEventListener("arbalette:state", (event) => resolve(event.detail), { once: true });
    fetch("/command/ready", { method: "POST" });
  });
  const title = `Renamed ${Math.random()}`;
  state.today[0] = { ...state.today[0], title };
  const start = performance.now();
  dispatchEvent(new CustomEvent("arbalette:state", { detail: state }));
  const tabs = document.querySelector(".today");
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
