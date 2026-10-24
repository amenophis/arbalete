// Checks the sidebar sections, selecting, closing, reopening and pinning tabs and favorites, moving
// between tabs and to favorites, creating, renaming and collapsing folders, dragging tabs
// between sections, painting the window with the Space's color, hiding and revealing the sidebar, keeping
// profiles' web data apart, switching Spaces, creating, renaming and coloring profiles, creating
// Spaces from the page after the last one and editing them, switching Spaces by number and by
// swiping between their pages, reopening closed tabs from the library before the first Space,
// showing the displayed profile's favorites, and recording each profile's history, in the real app.
// Usage: `mise run e2e`, which bundles the app and writes a session first.
// Rows are tracked as DOM elements: their titles change as pages load, React keeps the elements.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { connect, launch, pages, REVEAL, SIDEBAR, waitFor } from "./devtools.mjs";

const dataDir = process.argv[2];
if (!dataDir) throw new Error("usage: node e2e/tabs.mjs <data dir>");

// The sidebar shows one Space at a time: tabs of the others are found in the session file.
const readSession = () => JSON.parse(readFileSync(join(dataDir, "session.json"), "utf8")).sidebar;
const session = readSession();
const todayTab = (space, index) => session.nodes[space.today].children[index];
const tabUrl = (tab) => session.nodes[tab].kind.Tab.url;
const home = session.spaces[0];
const otherProfileSpace = session.spaces.find((space) => space.profile !== home.profile);

const app = launch(dataDir);
try {
  const sidebar = await connect(SIDEBAR);
  await sidebar.eval(waitFor(`document.querySelectorAll(".today .tab-row").length >= 10`));
  await sidebar.eval(`window.e2e = {
    // The displayed Space's page; its neighbors wait beside it for a swipe.
    page: () => document.querySelector(".space-page:not([inert])"),
    rows: () => [...e2e.page().querySelectorAll(".today .tab-row")],
    current: () => e2e.page().querySelector(".tab[aria-current]")?.closest(".tab-row"),
    above: (element) => !!(element.compareDocumentPosition(e2e.page().querySelector(".divider")) & Node.DOCUMENT_POSITION_FOLLOWING),
    send: (command) => fetch("/command/" + command, { method: "POST" }).then(() => {}),
    // Drags with pointer events, at a fraction of the target's width and height.
    pointer: (type, element, fx, fy) => {
      const rect = element.getBoundingClientRect();
      const init = { bubbles: true, button: 0, pointerId: 1, isPrimary: true, pointerType: "mouse",
        clientX: rect.left + rect.width * fx, clientY: rect.top + rect.height * fy };
      element.dispatchEvent(new PointerEvent(type, init));
    },
    press: (row) => { e2e.pointer("pointerdown", row, 0.5, 0.5); e2e.pointer("pointermove", row, 0.5, 0.9); },
    moveTo: (target, fx, fy) => e2e.pointer("pointermove", target, fx, fy),
    drag: (row, target, fx, fy) => { e2e.press(row); e2e.moveTo(target, fx, fy); e2e.pointer("pointerup", target, fx, fy); },
    indicator: () => document.querySelector(".drop-indicator"),
    // The tops of the favorites grid and the first pinned row, which must not move.
    tops: () => [".favorites", ".pinned .tab-row"]
      .map((selector) => e2e.page().querySelector(selector).getBoundingClientRect().top).join(),
    // Records the tops after every change of the sidebar, until the page has loaded.
    watchLayout: () => {
      const layout = { before: e2e.tops(), during: [], loading: false };
      const observer = new MutationObserver(() => {
        layout.loading ||= !!document.querySelector(".progress");
        layout.during.push(e2e.tops());
      });
      observer.observe(document.querySelector(".sidebar"), { childList: true, subtree: true, attributes: true });
      return layout;
    },
    has: (list, id) => [...list].some((element) => element.closest("li").dataset.id === id),
    // The commands the sidebar sends, the window's colors among them.
    sent: [],
    colors: () => e2e.sent.filter((url) => url.includes("/window/colors/")).at(-1),
  }`);
  await sidebar.eval(`{
    const fetchCommand = fetch;
    window.fetch = (url, init) => { e2e.sent.push(String(url)); return fetchCommand(url, init); };
  }`);

  async function check(description, action, condition, page = sidebar) {
    await sidebar.eval(action);
    try {
      await page.eval(waitFor(condition));
    } catch (error) {
      throw new Error(`not ok - ${description}\n  ${error.message}`);
    }
    console.log(`ok - ${description}`);
  }

  // The command painting the window with a Space's color (windowColors in ui/src/bridge.ts), from
  // the sidebar's own colors.
  const mix = (color, base, share) =>
    `${[1, 3, 5]
      .map((i) => {
        const [from, to] = [color, base].map((hex) => Number.parseInt(hex.slice(i, i + 2), 16));
        return Math.round((from * share + to * (100 - share)) / 100).toString(16).padStart(2, "0");
      })
      .join("")}`;
  const windowColors = async (color) => {
    const [end, panel] = await sidebar.eval(`(() => {
      const style = getComputedStyle(document.documentElement);
      return ["--window-end", "--panel"].map((name) => style.getPropertyValue(name).trim());
    })()`);
    return `/command/window/colors/${mix(color, end, 14)}/${mix(color, panel, 8)}`;
  };

  // The benchmark session pins 5 folders of 10 tabs (crates/arbalette-core/examples/bench-session.rs).
  await check(
    "the pinned section shows its folders and their tabs above the divider",
    ``,
    `(() => {
      const folders = [...e2e.page().querySelectorAll(".pinned .folder")];
      const tabs = folders[0]?.parentElement.querySelectorAll(".folder-tabs .tab-row") ?? [];
      return folders.length === 5 && folders[0].textContent.includes("Folder 1") && tabs.length === 10
        && e2e.above(folders[0]) && e2e.above(tabs[9]) && !e2e.above(e2e.rows()[0]);
    })()`,
  );
  await check("the first page finishes loading", ``, `!document.querySelector(".progress")`);
  await check(
    "loading a page moves nothing below the url bar",
    `e2e.layout = e2e.watchLayout(); e2e.rows()[1].querySelector(".tab").click()`,
    `e2e.layout.loading && !document.querySelector(".progress")
      && e2e.layout.during.every((tops) => tops === e2e.layout.before) && e2e.tops() === e2e.layout.before`,
  );
  await check(
    "selecting a tab marks it aria-current",
    `e2e.picked = e2e.rows()[2]; e2e.picked.querySelector(".tab").click()`,
    `e2e.current() === e2e.picked`,
  );
  await check(
    "the close button of a tab removes its row",
    `e2e.count = e2e.rows().length; e2e.closed = e2e.rows()[5]; e2e.closed.querySelector(".close").click()`,
    `!e2e.closed.isConnected && e2e.rows().length === e2e.count - 1`,
  );
  await check(
    "closing the selected tab with the close command selects its neighbor",
    `e2e.next = e2e.picked.nextElementSibling; e2e.send("close")`,
    `!e2e.picked.isConnected && e2e.current() === e2e.next`,
  );
  // The opened tab goes last: its neighbor is not its opener.
  await check(
    "the open command opens a tab after the others and selects it",
    `e2e.opener = e2e.current(); e2e.count = e2e.rows().length; e2e.send("open/" + encodeURIComponent("about:blank"))`,
    `e2e.rows().length === e2e.count + 1 && e2e.current() === e2e.rows().at(-1)`,
  );
  await check(
    "closing a tab returns to the tab it was opened from",
    `e2e.opened = e2e.current(); e2e.send("close")`,
    `!e2e.opened.isConnected && e2e.current() === e2e.opener`,
  );
  await check(
    "the reopen command brings the last closed tab back",
    `e2e.count = e2e.rows().length; e2e.send("reopen")`,
    `e2e.rows().length === e2e.count + 1 && e2e.current() === e2e.rows().at(-1) && e2e.current().textContent === e2e.opened.textContent`,
  );
  await check(
    "selecting a pinned tab inside a folder marks it aria-current",
    `e2e.picked = e2e.page().querySelectorAll(".pinned .folder-tabs .tab-row")[13]; e2e.picked.querySelector(".tab").click()`,
    `e2e.current() === e2e.picked`,
  );
  // The benchmark session has 6 favorites.
  await check(
    "the favorites show as a grid of 6 tiles above the pinned tabs",
    `e2e.tiles = () => [...e2e.page().querySelectorAll(".favorites .favorite")]`,
    `e2e.tiles().length === 6 && e2e.tiles()[0].getAttribute("aria-label") === "Favorite 1"
      && !!(e2e.tiles()[5].compareDocumentPosition(e2e.page().querySelector(".pinned")) & Node.DOCUMENT_POSITION_FOLLOWING)`,
  );
  await check(
    "the next command selects the next tab, inside the folder",
    `e2e.send("next")`,
    `e2e.current() === e2e.page().querySelectorAll(".pinned .folder-tabs .tab-row")[14]`,
  );
  await check(
    "the previous command selects the previous tab",
    `e2e.send("previous")`,
    `e2e.current() === e2e.picked`,
  );
  await check(
    "the favorite/1 command selects the first favorite",
    `e2e.send("favorite/1")`,
    `e2e.tiles()[0].getAttribute("aria-current") === "page" && !e2e.current()`,
  );
  await check(
    "the favorite/9 command selects the last favorite",
    `e2e.send("favorite/9")`,
    `e2e.tiles()[5].getAttribute("aria-current") === "page" && !e2e.tiles()[0].hasAttribute("aria-current")`,
  );
  await check(
    "the previous command from a favorite selects the last of today's tabs",
    `e2e.send("previous")`,
    `e2e.current() === e2e.rows().at(-1)`,
  );
  await check(
    "the next command wraps around to the first pinned tab",
    `e2e.send("next")`,
    `e2e.current() === e2e.page().querySelector(".pinned .tab-row")`,
  );
  await check(
    "selecting a favorite marks its tile aria-current and shows its url",
    `e2e.tile = e2e.tiles()[1]; e2e.tile.click()`,
    `e2e.tile.getAttribute("aria-current") === "page" && document.querySelector(".url")?.title.includes("favorite=2")`,
  );
  await check(
    "closing the selected favorite with the close command keeps its tile and deselects it",
    `e2e.send("close")`,
    `e2e.tile.isConnected && e2e.tiles().length === 6 && !e2e.tile.hasAttribute("aria-current") && e2e.current() === e2e.rows().at(-1)`,
  );
  // Pinning moves the row to another list, so rows are found again by position.
  await check(
    "the pin command moves the selected tab to the end of the pinned tabs",
    `e2e.pinnedRows = () => [...e2e.page().querySelectorAll(".pinned > .tab-row")];
     e2e.saved = document.querySelector(".url").title; e2e.count = e2e.rows().length; e2e.send("pin")`,
    `e2e.current() === e2e.pinnedRows().at(-1) && e2e.rows().length === e2e.count - 1 && e2e.above(e2e.current())`,
  );
  await check(
    "closing a pinned tab keeps its row, deselects it and shows its saved url",
    `e2e.pinned = e2e.current(); e2e.send("close")`,
    `e2e.pinned.isConnected && e2e.pinnedRows().at(-1) === e2e.pinned && !!e2e.current()
      && e2e.current() !== e2e.pinned && e2e.pinned.textContent.includes(e2e.saved)`,
  );
  await check(
    "selecting the closed pinned tab marks it aria-current",
    `e2e.pinned.querySelector(".tab").click()`,
    `e2e.current() === e2e.pinned`,
  );
  await check(
    "the pin command unpins a pinned tab to the top of today's tabs",
    `e2e.send("pin")`,
    `e2e.pinnedRows().length === 0 && e2e.current() === e2e.rows()[0] && e2e.rows().length === e2e.count`,
  );
  await check(
    "the new folder button adds a folder at the end of the pinned tabs and edits its title",
    `e2e.folders = () => [...e2e.page().querySelectorAll(".pinned > li > .folder")];
     e2e.page().querySelector(".new-folder").click()`,
    `(() => {
      const input = e2e.folders()[5]?.querySelector("input");
      return e2e.folders().length === 6 && input?.value === "New Folder" && document.activeElement === input;
    })()`,
  );
  await check(
    "Enter saves the edited folder title",
    `e2e.input = document.activeElement; e2e.input.value = "Reading";
     e2e.input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))`,
    `!e2e.input.isConnected && e2e.folders()[5].textContent.endsWith("Reading")`,
  );
  await check(
    "clicking a folder collapses it and hides its tabs",
    `e2e.folder = e2e.folders()[0]; e2e.folder.click()`,
    `e2e.folder.getAttribute("aria-expanded") === "false" && !e2e.folder.parentElement.querySelector(".folder-tabs")`,
  );
  await check(
    "clicking a collapsed folder expands it and shows its tabs again",
    `e2e.folder.click()`,
    `e2e.folder.getAttribute("aria-expanded") === "true"
      && e2e.folder.parentElement.querySelectorAll(".folder-tabs .tab-row").length === 10`,
  );
  await check(
    "dragging a tab dims it and shows a line where it would land",
    `e2e.rows()[0].scrollIntoView({ block: "center" }); e2e.dragged = e2e.rows()[3];
     e2e.press(e2e.dragged); e2e.moveTo(e2e.rows()[0], 0.5, 0.2)`,
    `e2e.dragged.classList.contains("dragged") && getComputedStyle(e2e.indicator()).display === "block"
      && Math.abs(e2e.indicator().getBoundingClientRect().top - e2e.rows()[0].getBoundingClientRect().top) <= 2`,
  );
  await check(
    "dropping a tab on the top of another moves it before that one",
    `e2e.pointer("pointerup", e2e.rows()[0], 0.5, 0.2)`,
    `e2e.rows()[0] === e2e.dragged && !e2e.dragged.classList.contains("dragged")
      && getComputedStyle(e2e.indicator()).display === "none"`,
  );
  await check(
    "dropping a tab of today on a pinned folder moves it into the folder",
    `e2e.reading = e2e.folders()[5]; e2e.reading.scrollIntoView({ block: "center" });
     e2e.moved = e2e.rows()[0].dataset.id; e2e.drag(e2e.rows()[0], e2e.reading, 0.5, 0.6)`,
    `e2e.has(e2e.reading.parentElement.querySelectorAll(".folder-tabs > li"), e2e.moved)
      && !e2e.has(e2e.rows(), e2e.moved) && e2e.above(e2e.reading.parentElement.querySelector(".folder-tabs > li"))`,
  );
  await check(
    "dropping a pinned tab on today's tabs moves it back to today",
    `e2e.drag(e2e.reading.parentElement.querySelector(".folder-tabs > li"), e2e.rows()[0], 0.5, 0.2)`,
    `e2e.rows()[0].dataset.id === e2e.moved && !e2e.reading.parentElement.querySelector(".folder-tabs > li")`,
  );
  await check(
    "dropping a tab after the last favorite makes it a favorite",
    `e2e.rows()[0].scrollIntoView({ block: "nearest" }); e2e.drag(e2e.rows()[0], e2e.tiles()[5], 0.8, 0.5)`,
    `e2e.tiles().length === 7 && e2e.tiles()[6].closest("li").dataset.id === e2e.moved && !e2e.has(e2e.rows(), e2e.moved)`,
  );
  await check(
    "the sidebar page does not scroll or bounce, only its tab list scrolls",
    `scrollBy(0, 500)`,
    `document.scrollingElement.scrollTop === 0
      && getComputedStyle(document.documentElement).overscrollBehaviorY === "none"
      && getComputedStyle(e2e.page().querySelector(".scroll")).overscrollBehaviorY === "none"`,
  );
  await check(
    "scrolling the tabs keeps the url bar in place",
    `e2e.top = document.querySelector(".url").getBoundingClientRect().top; e2e.page().querySelector(".scroll").scrollTop = 1000`,
    `e2e.page().querySelector(".scroll").scrollTop > 0 && document.scrollingElement.scrollTop === 0
      && document.querySelector(".url").getBoundingClientRect().top === e2e.top`,
  );

  const reveal = await connect(REVEAL);
  const content = await connect((await pages()).find((url) => !url.startsWith(SIDEBAR)));
  const width = (await content.eval("innerWidth")) + 260;
  // The card's margin, 9 pt from the window's edges and from the sidebar (CARD_MARGIN in shell.rs),
  // measured from the sidebar at the window's left edge: the card is a window of its own.
  const frame = await sidebar.eval(`({ x: screenX, y: screenY, height: innerHeight })`);
  await check(
    "the page is a card inset from the window's edges and from the sidebar",
    ``,
    `screenX === ${frame.x + 260 + 9} && screenY === ${frame.y + 9} && innerHeight === ${frame.height - 2 * 9}`,
    content,
  );
  // A web view reaching under the card gets no real mouse event on macOS, only synthesized ones
  // like these checks': the sidebar's view must keep its own width (SIDEBAR_WIDTH in shell.rs).
  await check(
    "the sidebar's view is the sidebar's width and paints its gradient",
    ``,
    `(() => {
      const background = getComputedStyle(document.querySelector(".sidebar"), "::before");
      return innerWidth === 260 && parseFloat(background.width) === 260
        && background.backgroundImage.startsWith("linear-gradient");
    })()`,
  );
  await check(
    "the sidebar's toolbar moves the window, not its controls",
    ``,
    `(() => {
      const region = (x, y) => getComputedStyle(document.elementFromPoint(x, y)).getPropertyValue("app-region");
      const back = document.querySelector('[aria-label="Back"]').getBoundingClientRect();
      return region(back.left - 4, back.top + 4) === "drag" && region(back.left + 4, back.top + 4) === "no-drag";
    })()`,
  );
  await check(
    "the sidebar toggle command hides the sidebar",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "hidden"`,
  );
  await check(
    "the page then takes the window's width within its margins, 9 pt on all four sides",
    ``,
    `innerWidth === ${width} && screenX === ${frame.x + 9} && screenY === ${frame.y + 9}
      && innerHeight === ${frame.height - 2 * 9}`,
    content,
  );
  await check(
    "the hidden sidebar's content slid out to the left",
    ``,
    `document.documentElement.hasAttribute("data-undocked")`,
  );
  await check(
    "the sidebar hidden, a strip of the revealed sidebar lines the window's left edge",
    ``,
    `document.visibilityState === "visible" && innerWidth === 4 && innerHeight === outerHeight`,
    reveal,
  );
  const mouse = (x) => reveal.call("Input.dispatchMouseEvent", { type: "mouseMoved", x, y: 200 });
  await mouse(2);
  // Floating 4 pt from the window's edges (FLOAT_INSET in shell.rs).
  const floating = `screenX === ${frame.x + 4} && screenY === ${frame.y + 4} && innerWidth === 260
    && innerHeight === ${frame.height - 2 * 4}`;
  await check(
    "the pointer at the window's left edge reveals the sidebar floating above the page",
    ``,
    `${floating} && document.querySelector(".space-page:not([inert])").querySelectorAll(".today .tab-row").length > 0`,
    reveal,
  );
  await check("the revealed sidebar does not resize the page", ``, `innerWidth === ${width}`, content);
  await check(
    "a tab clicked in the revealed sidebar is selected",
    ``,
    `(() => {
      const row = document.querySelector(".space-page:not([inert])").querySelectorAll(".today .tab-row")[1];
      if (!row.querySelector(".tab[aria-current]")) row.querySelector(".tab").click();
      return !!row.querySelector(".tab[aria-current]");
    })()`,
    reveal,
  );
  await mouse(-2);
  await sleep(200);
  await check("the pointer leaving the revealed sidebar towards the window's edge keeps it", ``, floating, reveal);
  // Back over the sidebar on its way to the page.
  await mouse(100);
  await mouse(600);
  await check("the pointer leaving the revealed sidebar hides it", ``, `innerWidth === 4`, reveal);
  await check(
    "the sidebar toggle command shows the sidebar again",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "visible" && e2e.current() === e2e.rows()[1]`,
  );
  await check("the revealed sidebar is hidden with it", ``, `document.visibilityState === "hidden"`, reveal);
  await check(
    "the page makes room for the sidebar again",
    ``,
    `innerWidth === ${width - 260} && screenX === ${frame.x + 260 + 9}`,
    content,
  );
  await check(
    "the shown sidebar's content slid back in",
    ``,
    `!document.documentElement.hasAttribute("data-undocked")
      && getComputedStyle(document.querySelector(".toolbar")).transform === "none"`,
  );
  await check("the sidebar toggle command hides the sidebar once more", `e2e.send("sidebar/toggle")`, `true`);
  await check("the strip lines the window's edge again", ``, `innerWidth === 4`, reveal);
  await mouse(2);
  await check("the pointer reveals the sidebar again", ``, floating, reveal);
  await check(
    "the sidebar toggle command docks the revealed sidebar",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "visible" && innerWidth === 260 && screenX === ${frame.x}`,
  );
  await check("the revealed sidebar is hidden once docked", ``, `document.visibilityState === "hidden"`, reveal);
  await check(
    "the page makes room for the docked sidebar",
    ``,
    `innerWidth === ${width - 260} && screenX === ${frame.x + 260 + 9}`,
    content,
  );

  // Selects a tab by id and checks a condition in its page once loaded and shown.
  async function checkPage(description, tab, condition) {
    await sidebar.eval(`e2e.send("select/${tab}")`);
    const page = await connect(tabUrl(tab));
    await check(description, ``, `document.visibilityState === "visible" && ${condition}`, page);
    return page;
  }
  const page = await checkPage("a tab of the first profile shows its page", todayTab(home, 45), `true`);
  await page.eval(`localStorage.setItem("profile", "first"); document.cookie = "profile=first; max-age=600"`);
  await checkPage(
    "a tab of another profile's Space on the same site sees none of the first profile's web data",
    todayTab(otherProfileSpace, 0),
    `localStorage.getItem("profile") === null && !document.cookie.includes("profile=")`,
  );
  await checkPage(
    "back on a tab of the first profile, its web data is still there",
    todayTab(home, 46),
    `localStorage.getItem("profile") === "first" && document.cookie.includes("profile=first")`,
  );

  // The urls in a profile's history, with their title and visit count.
  const history = (profile) => {
    const db = new DatabaseSync(join(dataDir, `Profile ${profile}`, "History.sqlite"), { readOnly: true });
    try {
      return db.prepare("SELECT url, title, visit_count FROM urls").all();
    } finally {
      db.close();
    }
  };
  // Waits until a profile's history meets `condition`.
  async function checkHistory(description, profile, condition) {
    const deadline = performance.now() + 10_000;
    while (!condition(history(profile))) {
      if (performance.now() > deadline) throw new Error(`not ok - ${description}`);
      await sleep(20);
    }
    console.log(`ok - ${description}`);
  }
  const visited = (urls, tab) => urls.find((entry) => entry.url === tabUrl(tab));
  await checkHistory("the first profile's history records its visits with their title", home.profile, (urls) =>
    [45, 46].every((index) => visited(urls, todayTab(home, index))?.title === "Example Domain"),
  );
  await checkHistory(
    "the other profile's history records only its own visits",
    otherProfileSpace.profile,
    (urls) => !!visited(urls, todayTab(otherProfileSpace, 0)) && !visited(urls, todayTab(home, 45)),
  );

  // Waits until the session file meets `condition`.
  async function checkSession(description, condition) {
    const deadline = performance.now() + 10_000;
    while (!condition(readSession())) {
      if (performance.now() > deadline) throw new Error(`not ok - ${description}`);
      await sleep(20);
    }
    console.log(`ok - ${description}`);
  }
  // Checks that the content view shows a tab's page.
  async function checkShown(description, tab) {
    const page = await connect(tabUrl(tab));
    await check(description, ``, `document.visibilityState === "visible"`, page);
  }
  const other = session.spaces.indexOf(otherProfileSpace);
  const firstPinned = (space) => session.nodes[session.nodes[space.pinned].children[0]].children[0];
  await check(
    "the switcher shows one icon per Space, the displayed one marked",
    `e2e.spaceIcons = () => [...document.querySelectorAll(".spaces .space")];
     e2e.currentId = () => e2e.current()?.dataset.id`,
    `e2e.spaceIcons().length === ${session.spaces.length} && e2e.spaceIcons()[0].getAttribute("aria-current") === "true"
      && e2e.spaceIcons().filter((icon) => icon.hasAttribute("aria-current")).length === 1`,
  );
  await check(
    "clicking a Space's icon shows only its pinned and today's tabs, with its selected tab",
    `e2e.spaceIcons()[${other}].click()`,
    `e2e.spaceIcons()[${other}].getAttribute("aria-current") === "true"
      && e2e.rows().length === 50 && e2e.rows()[0].dataset.id === "${todayTab(otherProfileSpace, 0)}"
      && e2e.page().querySelector(".pinned .folder-tabs .tab-row").dataset.id === "${firstPinned(otherProfileSpace)}"
      && !e2e.has(e2e.rows(), "${todayTab(home, 46)}") && e2e.currentId() === "${todayTab(otherProfileSpace, 0)}"`,
  );
  await checkShown("the content view shows that Space's selected tab", todayTab(otherProfileSpace, 0));
  await check(
    "selecting another tab of that Space marks it aria-current",
    `e2e.rows()[2].querySelector(".tab").click()`,
    `e2e.currentId() === "${todayTab(otherProfileSpace, 2)}"`,
  );
  await checkShown("the content view shows the tab selected in that Space", todayTab(otherProfileSpace, 2));
  await check(
    "switching back to the first Space shows its tabs and the tab it had selected",
    `e2e.spaceIcons()[0].click()`,
    `e2e.spaceIcons()[0].getAttribute("aria-current") === "true" && e2e.currentId() === "${todayTab(home, 46)}"
      && !e2e.has(e2e.rows(), "${todayTab(otherProfileSpace, 0)}")`,
  );
  await checkShown("the content view shows the first Space's tab again", todayTab(home, 46));
  await check(
    "switching to the other Space again restores the tab it had selected",
    `e2e.send("space/${otherProfileSpace.id}")`,
    `e2e.spaceIcons()[${other}].getAttribute("aria-current") === "true"
      && e2e.currentId() === "${todayTab(otherProfileSpace, 2)}"`,
  );
  await checkSession(
    "the session saves the displayed Space and its selected tab",
    (saved) =>
      saved.displayed === otherProfileSpace.id &&
      saved.spaces[other].selected === todayTab(otherProfileSpace, 2),
  );
  // The benchmark session gives the other profile 2 favorites of its own.
  const favoritesOf = (space) => session.profiles.find((profile) => profile.id === space.profile).favorites;
  const otherFavorites = session.nodes[favoritesOf(otherProfileSpace)].children;
  const tileIds = `e2e.tiles().map((tile) => tile.closest("li").dataset.id).join()`;
  // The page must have settled in place for the drop below to land on its tiles.
  await check(
    "the favorites grid shows the favorites of the displayed Space's profile",
    ``,
    `${tileIds} === "${otherFavorites.join()}" && e2e.page().getBoundingClientRect().left === 0`,
  );
  await check(
    "the favorite/1 command selects that profile's first favorite",
    `e2e.send("favorite/1")`,
    `e2e.tiles()[0].getAttribute("aria-current") === "page" && !e2e.current()`,
  );
  await checkPage(
    "that favorite opens in its profile's content view, apart from the first profile's web data",
    otherFavorites[0],
    `localStorage.getItem("profile") === null && !document.cookie.includes("profile=")`,
  );
  await check(
    "dropping a tab on that grid makes it a favorite of that profile",
    `e2e.moved = e2e.rows()[3].dataset.id; e2e.drag(e2e.rows()[3], e2e.tiles()[1], 0.8, 0.5)`,
    `e2e.tiles().length === 3 && e2e.tiles()[2].closest("li").dataset.id === e2e.moved`,
  );
  await checkSession("the session saves the dropped tab in that profile's favorites", (saved) =>
    saved.nodes[favoritesOf(otherProfileSpace)].children.includes(Number(todayTab(otherProfileSpace, 3))),
  );
  await check(
    "switching back to the first Space shows the first profile's favorites",
    `e2e.send("space/${home.id}")`,
    `e2e.spaceIcons()[0].getAttribute("aria-current") === "true" && e2e.tiles().length === 7
      && !e2e.has(e2e.tiles(), "${otherFavorites[0]}")`,
  );
  await check(
    "there the favorite/1 command selects the first profile's first favorite",
    `e2e.send("favorite/1")`,
    `e2e.tiles()[0].getAttribute("aria-current") === "page"
      && e2e.tiles()[0].closest("li").dataset.id === "${session.nodes[favoritesOf(home)].children[0]}"`,
  );
  await check(
    "switching to the other Space again shows its profile's favorites",
    `e2e.send("space/${otherProfileSpace.id}")`,
    `e2e.spaceIcons()[${other}].getAttribute("aria-current") === "true" && e2e.tiles().length === 3
      && e2e.tiles()[0].getAttribute("aria-current") === "page"`,
  );

  await check(
    "the footer menu's profiles item lists every profile by name",
    `e2e.names = () => [...document.querySelectorAll(".profile input")].map((input) => input.value);
     e2e.menu = async (name) => {
       document.querySelector(".new-toggle").click();
       await new Promise(requestAnimationFrame);
       [...document.querySelectorAll('[role="menuitem"]')].find((item) => item.textContent === name).click();
     };
     e2e.menu("Profiles…")`,
    `e2e.names().join() === "Default,Work"`,
  );
  await check(
    "the new profile button adds a profile and edits its name",
    `document.querySelector(".new-profile").click()`,
    `e2e.names().join() === "Default,Work,New Profile"
      && document.activeElement === document.querySelectorAll(".profile input")[2]`,
  );
  await check(
    "the new profile's Space shows in the switcher",
    ``,
    `e2e.spaceIcons().length === ${session.spaces.length + 1}`,
  );
  await check(
    "Enter saves the edited profile name",
    `e2e.input = document.activeElement; e2e.input.value = "Travel";
     e2e.input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))`,
    `e2e.names().join() === "Default,Work,Travel" && document.activeElement !== document.querySelectorAll(".profile input")[2]`,
  );
  await check(
    "picking a color marks it as the profile's",
    `document.querySelector('[aria-label="Color of Travel"] [aria-label="green"]').click()`,
    `document.querySelector('[aria-label="Color of Travel"] [aria-pressed="true"]')?.getAttribute("aria-label") === "green"`,
  );
  await checkSession("the session saves the new profile, its color and its Space", (saved) => {
    const travel = saved.profiles.find((profile) => profile.name === "Travel");
    return travel?.color === "green" && saved.spaces.some((space) => space.profile === travel.id);
  });

  // Fields save on blur, which only fires in a focused page: the window may not be the active one.
  await sidebar.call("Emulation.setFocusEmulationEnabled", { enabled: true });
  const spaceCount = session.spaces.length + 1;
  // Typed through DevTools, as the keyboard would.
  const type = (text) => sidebar.call("Input.insertText", { text });
  const pressEnter = async () => {
    const key = { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 };
    await sidebar.call("Input.dispatchKeyEvent", { type: "keyDown", ...key, text: "\r" });
    await sidebar.call("Input.dispatchKeyEvent", { type: "keyUp", ...key });
  };
  await check(
    "the footer menu's new Space item slides to the page creating one, untinted, its name field focused",
    `e2e.spaceField = (name) => document.querySelector('[aria-label="Space ' + name + '"]');
     e2e.newField = (name) => document.querySelector('[aria-label="New Space ' + name + '"]');
     e2e.newPage = () => document.querySelector(".new-space-page:not([inert])");
     e2e.last = () => e2e.spaceIcons().at(-1);
     e2e.tint = () => getComputedStyle(document.querySelector(".sidebar"));
     e2e.window = () => getComputedStyle(document.querySelector(".sidebar"), "::before").backgroundImage;
     e2e.background = e2e.window();
     e2e.menu("New Space")`,
    `e2e.newPage() && document.activeElement === e2e.newField("name")
      && document.querySelector(".pages").style.transform === "translateX(-${spaceCount * 100}%)"
      && document.querySelector(".sidebar").style.getPropertyValue("--tinted") === "0"
      && e2e.spaceIcons().length === ${spaceCount}`,
  );
  await type("Novels");
  await sidebar.eval(`e2e.newField("emoji").focus()`);
  await type("📖");
  await sidebar.eval(`document.querySelector('[aria-label="Color of the new Space"] [aria-label="blue"]').click();
    e2e.newField("name").focus()`);
  await pressEnter();
  await check(
    "Enter creates the Space with its name, emoji and color, and displays it empty",
    ``,
    `e2e.spaceIcons().length === ${spaceCount + 1} && e2e.last().getAttribute("aria-current") === "true"
      && e2e.last().getAttribute("aria-label") === "Novels" && e2e.last().textContent === "📖"
      && e2e.page()?.dataset.space && e2e.rows().length === 0 && !e2e.newPage()
      && document.querySelector(".pages").style.transform === "translateX(-${spaceCount * 100}%)"
      && e2e.tint().getPropertyValue("--space") === "#3e7bfa" && e2e.newField("name").value === ""`,
  );
  await check(
    "an empty Space's card and the window around it take the Space's color",
    ``,
    `e2e.colors() === ${JSON.stringify(await windowColors("#3e7bfa"))}`,
  );
  const blank = await connect("about:blank");
  await check("the page's view is hidden on a blank page", ``, `document.visibilityState === "hidden"`, blank);
  await check(
    "clicking the displayed Space's icon opens its editor",
    `e2e.last().click()`,
    `e2e.spaceField("name")?.value === "Novels"`,
  );
  await check(
    "Enter saves the Space's name in its editor",
    `e2e.input = e2e.spaceField("name"); e2e.input.focus(); e2e.input.value = "Reading";
     e2e.input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))`,
    `e2e.last().getAttribute("aria-label") === "Reading" && e2e.last().textContent === "📖"`,
  );
  await check(
    "the emoji field keeps one emoji, which the switcher shows instead of a dot",
    `e2e.input = e2e.spaceField("emoji"); e2e.input.focus(); e2e.input.value = "📚x";
     e2e.input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))`,
    `e2e.last().textContent === "📚" && e2e.spaceField("emoji").value === "📚"`,
  );
  await check(
    "picking a color tints the window around the page, behind the sidebar",
    `document.querySelector('[aria-label="Color of Reading"] [aria-label="orange"]').click()`,
    `e2e.tint().getPropertyValue("--space") === "#f76b15" && e2e.window().startsWith("linear-gradient")
      && e2e.window() !== e2e.background && e2e.tint().backgroundColor === "rgba(0, 0, 0, 0)"
      && e2e.colors() === ${JSON.stringify(await windowColors("#f76b15"))}`,
  );
  await checkSession("the session saves the new Space, its name, emoji and color", (saved) => {
    const space = saved.spaces.at(-1);
    return (
      saved.spaces.length === spaceCount + 1 &&
      saved.displayed === space.id &&
      space.profile === otherProfileSpace.profile &&
      space.title === "Reading" &&
      space.emoji === "📚" &&
      space.color === "orange"
    );
  });
  // The sidebar stays visible to DevTools with focus emulation: the floating one shows instead.
  await check(
    "the sidebar toggle beside an empty Space's panel floats the sidebar, inset from the window's edges",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "visible" && ${floating}`,
    reveal,
  );
  await mouse(600);
  await sleep(200);
  await check("the pointer leaving the floating sidebar keeps it", ``, `document.visibilityState === "visible" && ${floating}`, reveal);
  await check(
    "the sidebar toggle docks the floating sidebar",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "visible" && innerWidth === 260`,
  );
  await check("the floating sidebar is hidden once docked", ``, `document.visibilityState === "hidden"`, reveal);
  await check(
    "switching Space tints the window with that Space's color, the panel gone",
    `e2e.spaceIcons()[0].click()`,
    `e2e.spaceIcons()[0].getAttribute("aria-current") === "true" && e2e.tint().getPropertyValue("--space") === "#6e56cf"
      && document.querySelector(".pages").style.transform === "translateX(0%)"
      && e2e.colors() === ${JSON.stringify(await windowColors("#6e56cf"))}`,
  );

  // The tab list scrolls with a real wheel event, sent through DevTools, without switching Space.
  const scrollBox = await sidebar.eval(`(() => {
    const rect = e2e.page().querySelector(".scroll").getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  })()`);
  await sidebar.eval(`e2e.page().querySelector(".scroll").scrollTop = 0`);
  await sidebar.call("Input.dispatchMouseEvent", {
    type: "mouseWheel",
    ...scrollBox,
    deltaX: 0,
    deltaY: 300,
  });
  await check(
    "a vertical wheel scrolls the tab list and keeps the Space",
    ``,
    `e2e.page().querySelector(".scroll").scrollTop > 0 && e2e.spaceIcons()[0].getAttribute("aria-current") === "true"`,
  );
  await check(
    "the second Space's shortcut slides its page in",
    `e2e.send("space/number/2")`,
    `e2e.spaceIcons()[1].getAttribute("aria-current") === "true"
      && document.querySelector(".pages").style.transform === "translateX(-100%)"`,
  );
  // The app sends a trackpad swipe, from touch to lift, as arbalette:swipe events: `dx` is how
  // far the content would scroll, in points, `time` when, in milliseconds (app/src/swipe.rs).
  await sidebar.eval(`Object.assign(e2e, {
    gesture: (phase, dx, time, y = 300) =>
      dispatchEvent(new CustomEvent("arbalette:swipe", { detail: { phase, dx, y, time } })),
    // Touch, move \`events\` times \`interval\` ms apart, and lift unless \`hold\`.
    swipe: (dx, events, interval, hold) => {
      e2e.gesture("begin", dx, 0);
      for (let i = 1; i < events; i++) e2e.gesture("move", dx, i * interval);
      if (!hold) e2e.gesture("end", 0, events * interval);
    },
    // How far the pages are moved, in percent of a page.
    offset: () => Number.parseFloat(document.querySelector(".pages").style.transform.slice("translateX(".length)),
    displayed: (index) => e2e.spaceIcons()[index].getAttribute("aria-current") === "true"
      && e2e.page().dataset.space === String(e2e.spaces[index]),
    settled: (index) => e2e.displayed(index) && e2e.offset() === -index * 100,
  })`);
  await sidebar.eval(`e2e.spaces = [];
    addEventListener("arbalette:state", (event) => { e2e.state = event.detail; e2e.spaces = event.detail.spaces.map((space) => space.id) });
    e2e.send("ready")`);
  await check(
    "a Space next to the displayed one gets a color of its own",
    `e2e.send("space/color/" + e2e.spaces[2] + "/green")`,
    `e2e.state.spaces[2].color === "green"`,
  );
  // The window's margins, the first part of its colors, blend from one Space's to the next one's.
  const green = await windowColors("#30a46c");
  const margins = (colors) => colors.split("/").at(-2);
  await sidebar.eval(`e2e.margins = () => e2e.colors().split("/").at(-2); e2e.rest = e2e.margins()`);
  await check(
    "a slow swipe moves the pages with the fingers and blends the Spaces' tints and the window's",
    `e2e.swipe(-10, 15, 50, true)`,
    `e2e.offset() < -100 && e2e.offset() > -200
      && e2e.tint().getPropertyValue("--space") !== "#30a46c"
      && e2e.margins() !== e2e.rest && e2e.margins() !== "${margins(green)}"`,
  );
  await check(
    "lifting past half way displays the next Space, the window settling on its color",
    `e2e.swipe(-10, 15, 50)`,
    `e2e.settled(2) && e2e.tint().getPropertyValue("--space") === "#30a46c"
      && e2e.colors() === ${JSON.stringify(green)}`,
  );
  await check(
    "a short slow swipe goes back to the displayed Space",
    `e2e.swipe(-10, 5, 50, true)`,
    `e2e.offset() < -200`,
  );
  await check("lifting it settles the page back", `e2e.gesture("end", 0, 300)`, `e2e.settled(2)`);
  await check("a fast flick displays the next Space", `e2e.swipe(-15, 4, 10)`, `e2e.settled(3)`);
  await check(
    "a flick back displays the previous Space again",
    `e2e.swipe(15, 4, 10)`,
    `e2e.settled(2)`,
  );
  await check(
    "only the displayed Space's page and its neighbors exist, each once",
    ``,
    `(() => {
      const pages = [...document.querySelectorAll(".space-page")].map((page) => page.dataset.space);
      return pages.length === 3 && new Set(pages).size === 3
        && document.querySelectorAll(".space-page:not([inert])").length === 1
        && pages.join() === e2e.spaces.slice(1, 4).join();
    })()`,
  );
  await check(
    "the switcher slides to the Space clicked",
    `e2e.spaceIcons()[0].click()`,
    `e2e.displayed(0) && e2e.offset() > -100 && e2e.offset() < 0`,
  );
  await check("its page settles in view", ``, `e2e.settled(0)`);
  await check(
    "a closed tab leaves the displayed Space for the closed tabs",
    `e2e.count = e2e.rows().length; e2e.closing = e2e.rows().at(-1);
     e2e.closedText = e2e.closing.querySelector(".title").textContent;
     e2e.closing.querySelector(".close").click()`,
    `e2e.rows().length === e2e.count - 1 && e2e.state.closed.length > 0`,
  );
  await check(
    "on the way to the library the card follows the fingers and the first Space's page fades",
    `e2e.library = () => document.querySelector(".library:not([inert])");
     e2e.offsets = () => e2e.sent.filter((url) => url.includes("/card/offset/")).map((url) => Number(url.split("/").at(-1)));
     e2e.swipe(10, 4, 50, true)`,
    `e2e.offsets().at(-1) > 0 && e2e.offsets().at(-1) < 180
      && Number(document.querySelector('.space-page[data-space="' + e2e.spaces[0] + '"]').style.opacity) < 1`,
  );
  await check("the page keeps its size while the card moves", ``, `innerWidth === ${width - 260}`, content);
  await check("lifting early goes back, the card with it", `e2e.gesture("end", 0, 1000)`, `e2e.settled(0) && e2e.offsets().at(-1) === 0`);
  await check(
    "a swipe before the first Space shows the library listing it, the app keeping the first Space",
    `e2e.swipe(20, 15, 50)`,
    `e2e.library()?.querySelector(".tab")?.textContent === e2e.closedText && e2e.offset() === 100
      && e2e.state.space === e2e.spaces[0] && !e2e.page()
      && e2e.sent.at(-1).endsWith("/library/show")`,
  );
  // On the library, the sidebar and the window around it are untinted alike: no seam.
  const untinted = await sidebar.eval(`(() => {
    const style = getComputedStyle(document.documentElement);
    return style.getPropertyValue("--window-end").trim().slice(1);
  })()`);
  await check(
    "the library leaves the sidebar and the window's margins untinted alike",
    ``,
    `document.querySelector(".sidebar").style.getPropertyValue("--tinted") === "0"
      && e2e.margins() === "${untinted}"`,
  );
  await check(
    "the app widens the sidebar for the library, the card resting beside it",
    ``,
    `innerWidth === ${width - 440}`,
    content,
  );
  await check("the library is as wide as the sidebar", ``, `innerWidth === 440`);
  await check(
    "the library resists a swipe towards a page before it",
    `e2e.swipe(100, 20, 50, true)`,
    `e2e.offset() > 110 && e2e.offset() < 125`,
  );
  await check(
    "lifting it settles back on the library",
    `e2e.gesture("end", 0, 1000)`,
    `e2e.library() && e2e.offset() === 100`,
  );
  await check("the library's width comes back with it", ``, `innerWidth === 440`);
  await check(
    "reopening a tab from the library slides back to the displayed Space, where it opens",
    `e2e.closedCount = e2e.state.closed.length; e2e.library().querySelector(".tab").click()`,
    `e2e.settled(0) && !e2e.library() && e2e.rows().length === e2e.count
      && e2e.state.closed.length === e2e.closedCount - 1 && innerWidth === 260`,
  );
  await check("the card gets its place back beside the sidebar", ``, `innerWidth === ${width - 260}`, content);
  await check(
    "the footer's Library button slides to the library, the app keeping the first Space",
    `document.querySelector('.switcher [aria-label="Library"]').click()`,
    `e2e.library() && e2e.offset() === 100 && e2e.state.space === e2e.spaces[0]`,
  );
  await check("the first Space's dot slides back to its page", `e2e.spaceIcons()[0].click()`, `e2e.settled(0)`);
  const last = await sidebar.eval(`e2e.spaces.length - 1`);
  await check(
    "the last Space's shortcut slides its page in",
    `e2e.send("space/number/${last + 1}")`,
    `e2e.settled(${last})`,
  );
  const after = -(last + 1) * 100;
  await check(
    "a short pull past the last Space resists and brings out the + at the pointer",
    `e2e.bubble = () => document.querySelector(".new-bubble"); e2e.swipe(-10, 3, 50, true)`,
    `e2e.offset() < ${-last * 100} && e2e.offset() > ${-last * 100 - 10}
      && Number(e2e.bubble().style.opacity) > 0 && !e2e.bubble().hasAttribute("data-armed")
      && e2e.bubble().style.transform.includes("300px")`,
  );
  await check("released before it arms, it goes back", `e2e.gesture("end", 0, 1000)`, `e2e.settled(${last}) && e2e.bubble().style.opacity === "0"`);
  await check(
    "pulling further arms the +",
    `e2e.swipe(-20, 15, 50, true)`,
    `e2e.bubble().hasAttribute("data-armed") && e2e.bubble().style.opacity === "1"
      && getComputedStyle(e2e.bubble().firstChild).backgroundColor === "rgb(255, 255, 255)"`,
  );
  await check(
    "releasing it armed shows the page creating a Space",
    `e2e.gesture("end", 0, 1000)`,
    `e2e.newPage() && e2e.offset() === ${after}`,
  );
  await check("a swipe back settles on the last Space once more", `e2e.swipe(20, 15, 50)`, `e2e.settled(${last})`);
  await check(
    "a swipe past the last Space shows the page creating one, the app keeping the last Space",
    `e2e.swipe(-20, 15, 50)`,
    `e2e.newPage() && e2e.offset() === ${after} && document.activeElement === e2e.newField("name")
      && e2e.state.space === e2e.spaces[${last}] && !e2e.page()`,
  );
  await type("Unsaved");
  await check(
    "swiping back leaves the Spaces as they were and forgets the name typed",
    `e2e.swipe(20, 15, 50)`,
    `e2e.settled(${last}) && e2e.state.spaces.length === ${last + 1} && e2e.newField("name").value === ""`,
  );
  await check(
    "the page creating a Space resists a swipe towards the one after",
    `e2e.swipe(-20, 15, 50); e2e.swipe(-100, 20, 50, true)`,
    `e2e.offset() < ${after - 10} && e2e.offset() > ${after - 25}`,
  );
  await check(
    "lifting it settles back on the page creating a Space",
    `e2e.gesture("end", 0, 1000)`,
    `e2e.newPage() && e2e.offset() === ${after}`,
  );
  await check("a swipe back settles on the last Space", `e2e.swipe(20, 15, 50)`, `e2e.settled(${last})`);
  // Only the app's scroll events, which have phases, page between Spaces.
  const switcherBox = await sidebar.eval(`(() => {
    const rect = document.querySelector(".switcher").getBoundingClientRect();
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
  })()`);
  for (let i = 0; i < 10; i++) {
    await sidebar.call("Input.dispatchMouseEvent", { type: "mouseWheel", ...switcherBox, deltaX: 100, deltaY: 0 });
  }
  await sleep(300);
  await check("a horizontal wheel within the page alone keeps the Space", ``, `e2e.settled(${last})`);
} finally {
  app.kill();
}
