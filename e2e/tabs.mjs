// Checks the sidebar sections, selecting, closing, reopening and pinning tabs and favorites, moving
// between tabs and to favorites, creating, renaming and collapsing folders, dragging tabs
// between sections, hiding and revealing the sidebar, keeping profiles' web data apart, switching
// Spaces, and creating, renaming and coloring profiles, in the real app.
// Usage: `mise run e2e`, which bundles the app and writes a session first.
// Rows are tracked as DOM elements: their titles change as pages load, React keeps the elements.

import { readFileSync } from "node:fs";
import { join } from "node:path";
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
    rows: () => [...document.querySelectorAll(".today .tab-row")],
    current: () => document.querySelector(".tab[aria-current]")?.closest(".tab-row"),
    above: (element) => !!(element.compareDocumentPosition(document.querySelector(".divider")) & Node.DOCUMENT_POSITION_FOLLOWING),
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
      .map((selector) => document.querySelector(selector).getBoundingClientRect().top).join(),
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

  // The benchmark session pins 5 folders of 10 tabs (crates/arbalette-core/examples/bench-session.rs).
  await check(
    "the pinned section shows its folders and their tabs above the divider",
    ``,
    `(() => {
      const folders = [...document.querySelectorAll(".pinned .folder")];
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
    `e2e.picked = document.querySelectorAll(".pinned .folder-tabs .tab-row")[13]; e2e.picked.querySelector(".tab").click()`,
    `e2e.current() === e2e.picked`,
  );
  // The benchmark session has 6 favorites.
  await check(
    "the favorites show as a grid of 6 tiles above the pinned tabs",
    `e2e.tiles = () => [...document.querySelectorAll(".favorites .favorite")]`,
    `e2e.tiles().length === 6 && e2e.tiles()[0].getAttribute("aria-label") === "Favorite 1"
      && !!(e2e.tiles()[5].compareDocumentPosition(document.querySelector(".pinned")) & Node.DOCUMENT_POSITION_FOLLOWING)`,
  );
  await check(
    "the next command selects the next tab, inside the folder",
    `e2e.send("next")`,
    `e2e.current() === document.querySelectorAll(".pinned .folder-tabs .tab-row")[14]`,
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
    `e2e.current() === document.querySelector(".pinned .tab-row")`,
  );
  await check(
    "selecting a favorite marks its tile aria-current and shows its url",
    `e2e.tile = e2e.tiles()[1]; e2e.tile.click()`,
    `e2e.tile.getAttribute("aria-current") === "page" && document.querySelector(".url")?.textContent.includes("favorite=2")`,
  );
  await check(
    "closing the selected favorite with the close command keeps its tile and deselects it",
    `e2e.send("close")`,
    `e2e.tile.isConnected && e2e.tiles().length === 6 && !e2e.tile.hasAttribute("aria-current") && e2e.current() === e2e.rows().at(-1)`,
  );
  // Pinning moves the row to another list, so rows are found again by position.
  await check(
    "the pin command moves the selected tab to the end of the pinned tabs",
    `e2e.pinnedRows = () => [...document.querySelectorAll(".pinned > .tab-row")];
     e2e.saved = document.querySelector(".url").textContent; e2e.count = e2e.rows().length; e2e.send("pin")`,
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
    `e2e.folders = () => [...document.querySelectorAll(".pinned > li > .folder")];
     document.querySelector(".new-folder").click()`,
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
      && getComputedStyle(document.querySelector(".scroll")).overscrollBehaviorY === "none"`,
  );
  await check(
    "scrolling the tabs keeps the url bar in place",
    `e2e.top = document.querySelector(".url").getBoundingClientRect().top; document.querySelector(".scroll").scrollTop = 1000`,
    `document.querySelector(".scroll").scrollTop > 0 && document.scrollingElement.scrollTop === 0
      && document.querySelector(".url").getBoundingClientRect().top === e2e.top`,
  );

  const reveal = await connect(REVEAL);
  const content = await connect((await pages()).find((url) => !url.startsWith(SIDEBAR)));
  const width = (await content.eval("innerWidth")) + 260;
  await check(
    "the sidebar toggle command hides the sidebar",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "hidden"`,
  );
  await check("the page then takes the whole window width", ``, `innerWidth === ${width}`, content);
  await check(
    "the sidebar hidden, a strip of the revealed sidebar lines the window's left edge",
    ``,
    `document.visibilityState === "visible" && innerWidth === 4 && innerHeight === outerHeight`,
    reveal,
  );
  const mouse = (x) => reveal.call("Input.dispatchMouseEvent", { type: "mouseMoved", x, y: 200 });
  await mouse(2);
  await check(
    "the pointer at the window's left edge reveals the sidebar above the page",
    ``,
    `innerWidth === 260 && document.querySelectorAll(".today .tab-row").length > 0`,
    reveal,
  );
  await check("the revealed sidebar does not resize the page", ``, `innerWidth === ${width}`, content);
  await check(
    "a tab clicked in the revealed sidebar is selected",
    ``,
    `(() => {
      const row = document.querySelectorAll(".today .tab-row")[1];
      if (!row.querySelector(".tab[aria-current]")) row.querySelector(".tab").click();
      return !!row.querySelector(".tab[aria-current]");
    })()`,
    reveal,
  );
  await mouse(600);
  await check("the pointer leaving the revealed sidebar hides it", ``, `innerWidth === 4`, reveal);
  await check(
    "the sidebar toggle command shows the sidebar again",
    `e2e.send("sidebar/toggle")`,
    `document.visibilityState === "visible" && e2e.current() === e2e.rows()[1]`,
  );
  await check("the revealed sidebar is hidden with it", ``, `document.visibilityState === "hidden"`, reveal);

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
      && document.querySelector(".pinned .folder-tabs .tab-row").dataset.id === "${firstPinned(otherProfileSpace)}"
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

  await check(
    "the profiles button lists every profile by name",
    `e2e.names = () => [...document.querySelectorAll(".profile input")].map((input) => input.value);
     document.querySelector(".profiles-toggle").click()`,
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
} finally {
  app.kill();
}
