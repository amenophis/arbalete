// Checks that the first launch imports Arc's sidebar, and that later launches keep the session,
// in the real app, on the fictitious Arc data of crates/arbalette-core/tests/fixtures/arc.
// Usage: `mise run e2e`, with an empty directory outside the repository.

import { once } from "node:events";
import { cpSync, existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { connect, launch, SIDEBAR, waitFor } from "./devtools.mjs";

const dir = process.argv[2];
if (!dir) throw new Error("usage: node e2e/arc.mjs <empty dir>");
// Copied: macOS asks before an app reads the Documents folder, where the repository may sit.
const arcDir = join(dir, "arc");
cpSync(resolve("crates/arbalette-core/tests/fixtures/arc"), arcDir, { recursive: true });
const dataDir = join(dir, "data");
const sessionFile = join(dataDir, "session.json");
const readSession = () => JSON.parse(readFileSync(sessionFile, "utf8")).sidebar;

async function check(page, description, action, condition) {
  await page.eval(action);
  try {
    await page.eval(waitFor(condition));
  } catch (error) {
    throw new Error(`not ok - ${description}\n  ${error.message}`);
  }
  console.log(`ok - ${description}`);
}

async function checkSession(description, condition) {
  const deadline = performance.now() + 10_000;
  while (!(existsSync(sessionFile) && condition(readSession()))) {
    if (performance.now() > deadline) throw new Error(`not ok - ${description}`);
    await sleep(20);
  }
  console.log(`ok - ${description}`);
}

const helpers = `window.e2e = {
  spaces: () => [...document.querySelectorAll(".spaces .space")].map((icon) => icon.getAttribute("aria-label")).join(),
  tiles: () => [...document.querySelectorAll(".favorites .favorite")].map((tile) => tile.getAttribute("aria-label")).join(),
  rows: () => [...document.querySelectorAll(".today .tab-row")].map((row) => row.dataset.id).join(),
}`;
// The imported tabs of today come first: the app opens its start page after them.
const importedToday = (session) => session.nodes[session.spaces[0].today].children.slice(0, 3).join();

let app = launch(dataDir, arcDir);
let closed;
try {
  await checkSession(
    "the first launch saves a session imported from Arc, with its profiles and Spaces",
    (session) =>
      session.profiles.map((profile) => profile.name).join() === "Personal,Work" &&
      session.spaces.map((space) => space.title).join() === "Home,Space,Office",
  );
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(helpers);
  await check(sidebar, "the switcher shows the imported Spaces", ``, `e2e.spaces() === "Home,Space,Office"`);
  await check(
    sidebar,
    "the first Space shows its imported pinned folder and tab",
    ``,
    `document.querySelector(".pinned .folder")?.textContent.includes("Docs")
      && document.querySelectorAll(".pinned > li").length === 2`,
  );
  await check(
    sidebar,
    "the first Space shows its imported tabs of today",
    ``,
    `e2e.rows().startsWith("${importedToday(readSession())}")`,
  );
  await check(
    sidebar,
    "the favorites are those of the first Space's profile",
    ``,
    `document.querySelectorAll(".favorites .favorite").length === 2`,
  );
  closed = readSession().nodes[readSession().spaces[0].today].children[0];
  await check(
    sidebar,
    "closing an imported tab removes it",
    `document.querySelector('[data-id="${closed}"] .close').click()`,
    `!document.querySelector('[data-id="${closed}"]')`,
  );
  await check(
    sidebar,
    "the Space of the other profile shows that profile's favorite",
    `document.querySelectorAll(".spaces .space")[2].click()`,
    `e2e.tiles() === "Tracker"`,
  );
  await check(
    sidebar,
    "the profiles list shows the imported names",
    `document.querySelector(".profiles-toggle").click()`,
    `[...document.querySelectorAll(".profile input")].map((input) => input.value).join() === "Personal,Work"`,
  );
  await checkSession("the session saves the closed tab", (session) => !session.nodes[closed]);
} finally {
  app.kill();
}
await once(app, "exit");

app = launch(dataDir, arcDir);
try {
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(helpers);
  await check(
    sidebar,
    "a later launch restores the session instead of importing Arc again",
    ``,
    `e2e.spaces() === "Home,Space,Office" && e2e.tiles() === "Tracker"`,
  );
  await checkSession(
    "the session keeps its profiles and the tab closed before",
    (session) => session.profiles.length === 2 && session.spaces.length === 3 && !session.nodes[closed],
  );
} finally {
  app.kill();
}
