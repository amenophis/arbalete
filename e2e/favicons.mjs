// Checks that rows and favorites show their page's favicon from the app's own cache: once a local
// page reports it, then after a restart with the page never loaded and its server gone, and that
// the selected favorite takes the icon's color, which needs the icon's pixels.
// Usage: `mise run e2e`, with an empty directory outside the repository.

import { createHash } from "node:crypto";
import { once } from "node:events";
import { existsSync, readdirSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { connect, launch, quit, SIDEBAR, waitFor } from "./devtools.mjs";
import { png } from "./png.mjs";

const dataDir = process.argv[2];
if (!dataDir) throw new Error("usage: node e2e/favicons.mjs <empty dir>");

const GREEN = [0x30, 0xa4, 0x6c];
const icon = png(32, GREEN);
const server = createServer((request, response) => {
  if (request.url === "/icon.png") {
    response.writeHead(200, { "content-type": "image/png" });
    return response.end(icon);
  }
  const title = request.url === "/tinted" ? "Tinted" : "Iconic";
  response.writeHead(200, { "content-type": "text/html" });
  response.end(`<!doctype html><title>${title}</title><link rel="icon" href="/icon.png">`);
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
const origin = `http://127.0.0.1:${server.address().port}`;

const helpers = `window.e2e = {
  send: (command) => fetch("/command/" + command, { method: "POST" }).then(() => {}),
  state: () => new Promise((resolve) => {
    addEventListener("arbalete:state", (event) => resolve(event.detail), { once: true });
    e2e.send("ready");
  }),
  page: () => document.querySelector(".space-page:not([inert])"),
  row: (title) => [...e2e.page().querySelectorAll(".tab-row")].find((row) => row.textContent.includes(title)),
  tile: (title) => e2e.page().querySelector('.favorite[aria-label="' + title + '"]'),
  // An icon served by the app from its cache, loaded.
  cached: (img) => !!img && img.src.startsWith("https://arbalete.ui/favicon/") && img.complete && img.naturalWidth > 0,
}`;

async function check(page, description, action, condition) {
  await page.eval(action);
  try {
    await page.eval(waitFor(condition));
  } catch (error) {
    throw new Error(`not ok - ${description}\n  ${error.message}`);
  }
  console.log(`ok - ${description}`);
}

let app = launch(dataDir);
try {
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(helpers);
  await sidebar.eval(waitFor(`!!e2e.page()?.querySelector(".today .tab-row")`));
  const start = await sidebar.eval(`e2e.page().querySelector(".today .tab-row").dataset.id`);
  await check(
    sidebar,
    "a page's favicon is downloaded and shown from the app's cache",
    `e2e.send("open/" + encodeURIComponent(${JSON.stringify(`${origin}/iconic`)}))`,
    `e2e.cached(e2e.row("Iconic")?.querySelector("img.favicon"))`,
  );
  const profiles = readdirSync(dataDir).filter((name) => name.startsWith("Profile "));
  const files = profiles.flatMap((profile) => {
    const dir = join(dataDir, profile, "Sidebar Favicons");
    return existsSync(dir) ? readdirSync(dir) : [];
  });
  // Named after the md5 of the icon's url.
  const cached = `${createHash("md5").update(`${origin}/icon.png`).digest("hex")}.png`;
  if (!files.includes(cached)) throw new Error(`not ok - the icon is cached: ${files}`);
  console.log("ok - the icon is cached in its profile's directory");

  await check(
    sidebar,
    "another page with the same icon shows it",
    `e2e.send("open/" + encodeURIComponent(${JSON.stringify(`${origin}/tinted`)}))`,
    `e2e.cached(e2e.row("Tinted")?.querySelector("img.favicon"))`,
  );
  await sidebar.eval(`e2e.state().then((state) => {
    const page = state.pages.find((page) => page.space === state.space);
    return e2e.send("move/" + e2e.row("Tinted").dataset.id + "/" + page.roots.favorites + "/0");
  })`);
  await check(
    sidebar,
    "the selected favorite takes the color of its cached icon",
    ``,
    `e2e.tile("Tinted")?.style.getPropertyValue("--tile-color") === "#30a46c"`,
  );
  await sidebar.eval(`e2e.send("select/${start}")`);
  await sidebar.eval(
    waitFor(`!!e2e.page().querySelector('.tab-row[data-id="${start}"] .tab[aria-current]')`),
  );
  await quit(app);
} finally {
  app.kill();
}

// Neither page loads after the restart, and their server is gone.
server.close();
app = launch(dataDir);
try {
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(helpers);
  await check(
    sidebar,
    "after a restart, a row and a favorite never loaded show their cached icon",
    ``,
    `e2e.cached(e2e.row("Iconic")?.querySelector("img.favicon"))
      && e2e.cached(e2e.tile("Tinted")?.querySelector("img.favicon"))`,
  );
} finally {
  app.kill();
}
await once(app, "exit");
