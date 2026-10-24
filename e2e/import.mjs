// Checks the import assistant in the real app: shown on the first launch with the browsers of a
// fictitious home (Arc's fixture from crates/arbalette-core/tests/fixtures/arc, a Chrome profile
// and a Safari that needs Full Disk Access), importing Arc and Chrome, reopening it from the
// "Import…" command to merge into an existing profile, skipping it, and later launches keeping
// the session. Chrome's history is large, to check that the window keeps answering while it is
// imported.
// Usage: `mise run e2e`, with an empty directory outside the repository.

import { once } from "node:events";
import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { connect, IMPORT, launch, pages, SIDEBAR, waitFor } from "./devtools.mjs";

const dir = process.argv[2];
if (!dir) throw new Error("usage: node e2e/import.mjs <empty dir>");

// The home's browsers. Copied: macOS asks before an app reads the Documents folder, where the
// repository may sit.
const home = join(dir, "home");
const support = join(home, "Library/Application Support");
const arc = join(support, "Arc");
cpSync(resolve("crates/arbalette-core/tests/fixtures/arc"), arc, { recursive: true });
for (const profile of ["Default", "Profile 1"]) mkdirSync(join(arc, "User Data", profile));
const chrome = join(support, "Google/Chrome");
mkdirSync(join(chrome, "Default"), { recursive: true });
writeFileSync(
  join(chrome, "Local State"),
  JSON.stringify({ profile: { info_cache: { Default: { name: "Personal" } } } }),
);
writeFileSync(
  join(chrome, "Default/Bookmarks"),
  JSON.stringify({
    roots: {
      bookmark_bar: {
        children: [{ name: "Mail", type: "url", url: "https://mail.example.com/" }],
      },
      other: {
        name: "Other bookmarks",
        children: [{ name: "News", type: "url", url: "https://news.example.net/" }],
      },
    },
  }),
);
const chromeHistory = new DatabaseSync(join(chrome, "Default/History"));
chromeHistory.exec(`
  CREATE TABLE urls (url TEXT, title TEXT, visit_count INTEGER, last_visit_time INTEGER, hidden INTEGER);
  INSERT INTO urls VALUES ('https://shop.example.org/', 'Shop', 4, 13350000000000000, 0);
`);
const HISTORY_SIZE = 200_000;
const insert = chromeHistory.prepare("INSERT INTO urls VALUES (?, ?, 1, ?, 0)");
chromeHistory.exec("BEGIN");
for (let i = 0; i < HISTORY_SIZE; i++) {
  insert.run(`https://site${i}.example.com/`, `Site ${i}`, 13350000000000000 + i);
}
chromeHistory.exec("COMMIT");
chromeHistory.close();
// Unreadable, as Safari's data is to an app without Full Disk Access.
mkdirSync(join(home, "Library/Safari"));
writeFileSync(join(home, "Library/Safari/Bookmarks.plist"), "");
chmodSync(join(home, "Library/Safari/Bookmarks.plist"), 0o000);

async function check(page, description, action, condition) {
  await page.eval(action);
  try {
    await page.eval(waitFor(condition));
  } catch (error) {
    throw new Error(`not ok - ${description}\n  ${error.message}`);
  }
  console.log(`ok - ${description}`);
}

async function until(description, condition) {
  const deadline = performance.now() + 10_000;
  while (!(await condition())) {
    if (performance.now() > deadline) throw new Error(`not ok - ${description}`);
    await sleep(20);
  }
  console.log(`ok - ${description}`);
}

const sessionFile = (dataDir) => join(dataDir, "session.json");
const readSession = (dataDir) => JSON.parse(readFileSync(sessionFile(dataDir), "utf8")).sidebar;
const checkSession = (dataDir, description, condition) =>
  until(description, () => existsSync(sessionFile(dataDir)) && condition(readSession(dataDir)));
const names = (items, key) => items.map((item) => item[key]).join();
const children = (session, id) =>
  session.nodes[id].children.map((child) => session.nodes[child].kind);

const helpers = `window.e2e = {
  send: (command) => fetch("/command/" + command, { method: "POST" }).then(() => {}),
  box: (name) => document.querySelector('[aria-label="' + name + '"]'),
  // Changes a select the way a user does, which React hears.
  choose: (name, value) => {
    const select = e2e.box(name);
    select.value = value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
  },
  spaces: () => [...document.querySelectorAll(".spaces .space")].map((icon) => icon.getAttribute("aria-label")).join(),
  // The sidebar's displayed Space's page; its neighbors wait beside it for a swipe.
  page: () => document.querySelector(".space-page:not([inert])"),
  tiles: () => [...e2e.page().querySelectorAll(".favorites .favorite")].map((tile) => tile.getAttribute("aria-label")).join(),
  // How long the app takes to answer a command with its state, in ms: the UI thread is free.
  roundTrip: () => new Promise((resolve) => {
    const start = performance.now();
    addEventListener("arbalette:state", () => resolve(performance.now() - start), { once: true });
    e2e.send("ready");
  }),
}`;

const dataDir = join(dir, "data");
let app = launch(dataDir, home);
try {
  const assistant = await connect(IMPORT);
  await assistant.eval(helpers);
  await check(
    assistant,
    "the first launch shows the assistant with Arc's profiles picked, and Chrome's not",
    ``,
    `e2e.box("Import Arc Personal")?.checked && e2e.box("Import Arc Work")?.checked
      && e2e.box("Import Chrome Personal")?.checked === false`,
  );
  await check(
    assistant,
    "the first launch offers no profile to merge into",
    ``,
    `!document.querySelector('select[aria-label^="Destination"]')`,
  );
  await check(
    assistant,
    "Safari asks for Full Disk Access instead of offering its profile",
    ``,
    `document.querySelector(".import-access")?.textContent.includes("Full Disk Access")
      && !e2e.box("Import Safari Default")
      && [...document.querySelectorAll(".import-access button")].some((b) => b.textContent === "Open System Settings")`,
  );
  if (existsSync(sessionFile(dataDir)))
    throw new Error("not ok - the session is saved before a choice");
  console.log("ok - nothing is saved before a choice");
  await assistant.eval(`e2e.box("Import Chrome Personal").click()`);
  await assistant.eval(waitFor(`e2e.box("Import Chrome Personal").checked`));
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(helpers);
  const started = performance.now();
  await assistant.eval(`document.querySelector('button[type="submit"]').click()`);
  await check(
    assistant,
    "the assistant shows the import running, with its buttons disabled",
    ``,
    `[...document.querySelectorAll("button")].every((b) => b.matches(":disabled"))
      && document.querySelector('button[type="submit"]').textContent === "Importing…"`,
  );
  // The session is saved once the import is applied.
  const roundTrips = [];
  while (!existsSync(sessionFile(dataDir))) roundTrips.push(await sidebar.eval(`e2e.roundTrip()`));
  const imported = performance.now() - started;
  const slowest = Math.max(...roundTrips);
  if (roundTrips.length === 0 || slowest > 250) {
    throw new Error(
      `not ok - the window answers while the import runs (${roundTrips.length} round trips, slowest ${Math.round(slowest)} ms)`,
    );
  }
  console.log(
    `ok - the window answers while the import runs: ${roundTrips.length} round trips, slowest ${Math.round(slowest)} ms, import applied after ${Math.round(imported)} ms`,
  );
  await checkSession(
    dataDir,
    "importing Arc and Chrome replaces the default session with their profiles and Spaces",
    (session) =>
      names(session.profiles, "name") === "Personal (Arc),Work (Arc),Chrome" &&
      names(session.spaces, "title") === "Home,Space,Office,Chrome",
  );
  const session = readSession(dataDir);
  const [homeSpace, , , chromeSpace] = session.spaces;
  if (session.nodes[homeSpace.today].children.length !== 3) {
    throw new Error("not ok - the start page is not added to the imported tabs of today");
  }
  console.log("ok - the start page is not added to the imported tabs of today");
  const pinnedFolders = children(session, chromeSpace.pinned).map((kind) => kind.Folder?.title);
  if (pinnedFolders.join() !== "Other bookmarks")
    throw new Error("not ok - Chrome's folders are pinned");
  console.log("ok - Chrome's other bookmarks become a pinned folder of its Space");

  await check(
    sidebar,
    "the sidebar shows the imported Spaces, and an imported tab of the first",
    ``,
    `e2e.spaces() === "Home,Space,Office,Chrome"
      && document.querySelector(".url").textContent.startsWith("https://docs.example.com/guide")`,
  );
  await check(
    sidebar,
    "the first Space shows its imported pinned folder and the favorites of its profile",
    ``,
    `e2e.page().querySelector(".pinned .folder")?.textContent.includes("Docs") && e2e.tiles().split(",").length === 2`,
  );
  await sidebar.eval(`e2e.urlHeight = document.querySelector(".url").offsetHeight`);
  await check(
    sidebar,
    "Chrome's Space shows its bookmarks bar as favorites, with the url bar in its place",
    `document.querySelectorAll(".spaces .space")[3].click()`,
    `e2e.tiles() === "Mail" && document.querySelector(".url").textContent === ""
      && document.querySelector(".url").offsetHeight === e2e.urlHeight`,
  );
  const chromeProfile = session.profiles[2].id;
  await until("Chrome's history is merged into its profile's", () => {
    const file = join(dataDir, `Profile ${chromeProfile}`, "History.sqlite");
    if (!existsSync(file)) return false;
    const db = new DatabaseSync(file, { readOnly: true });
    try {
      const row = db
        .prepare("SELECT visit_count FROM urls WHERE url = ?")
        .get("https://shop.example.org/");
      const { count } = db.prepare("SELECT COUNT(*) AS count FROM urls").get();
      return row?.visit_count === 4 && count === HISTORY_SIZE + 1;
    } catch {
      return false;
    } finally {
      db.close();
    }
  });
  console.log(`  (history merged ${Math.round(performance.now() - started)} ms after the click)`);

  await check(
    assistant,
    'the "Import…" command reopens the assistant, which offers the profiles to merge into',
    `e2e.send("import/show")`,
    `document.querySelector('select[aria-label="Destination of Chrome Personal"]')?.options.length === 4`,
  );
  await assistant.eval(`e2e.box("Import Arc Personal").click(); e2e.box("Import Arc Work").click();
    e2e.box("Import Chrome Personal").click()`);
  await assistant.eval(
    waitFor(`!e2e.box("Import Arc Personal").checked && e2e.box("Import Chrome Personal").checked`),
  );
  await assistant.eval(`e2e.choose("Destination of Chrome Personal", "${session.profiles[0].id}");
    e2e.choose("Bookmarks bar of Chrome Personal", "pinnedFolder")`);
  await assistant.eval(
    waitFor(`e2e.box("Bookmarks bar of Chrome Personal").value === "pinnedFolder"`),
  );
  await assistant.eval(`document.querySelector('button[type="submit"]').click()`);
  await checkSession(
    dataDir,
    "merging Chrome into an existing profile pins its bar in the profile's first Space",
    (session) =>
      session.profiles.length === 3 &&
      session.spaces.length === 4 &&
      children(session, session.spaces[0].pinned).some(
        (kind) => kind.Folder?.title === "Bookmarks bar",
      ),
  );
} finally {
  app.kill();
}
await once(app, "exit");

app = launch(dataDir, home);
try {
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(helpers);
  await check(
    sidebar,
    "a later launch restores the session",
    ``,
    `e2e.spaces() === "Home,Space,Office,Chrome"`,
  );
  await sleep(500);
  if ((await pages()).includes(IMPORT))
    throw new Error("not ok - a later launch shows the assistant");
  console.log("ok - a later launch does not show the assistant");
} finally {
  app.kill();
}
await once(app, "exit");

const skipped = join(dir, "skipped");
app = launch(skipped, home);
try {
  const assistant = await connect(IMPORT);
  await assistant.eval(helpers);
  await assistant.eval(waitFor(`!!e2e.box("Import Arc Personal")`));
  await assistant.eval(
    `[...document.querySelectorAll("button")].find((b) => b.textContent === "Not now").click()`,
  );
  await checkSession(
    skipped,
    "skipping the assistant keeps the default session, with the start page",
    (session) =>
      names(session.profiles, "name") === "Default" &&
      names(session.spaces, "title") === "Home" &&
      session.nodes[session.spaces[0].today].children.length === 1,
  );
} finally {
  app.kill();
}
await once(app, "exit");
