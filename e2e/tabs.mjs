// Checks the sidebar sections, selecting, closing, reopening and pinning tabs and favorites, moving
// between tabs and to favorites, creating, renaming and collapsing folders, dragging tabs and favorites with the mouse
// between sections and into splits, painting the window with the Space's color, hiding and revealing the sidebar, keeping
// profiles' web data apart, switching Spaces, creating, renaming and coloring profiles, creating
// Spaces from the page after the last one and editing them, switching Spaces by number and by
// swiping between their pages, reopening closed tabs from the library before the first Space and
// searching them, managing Spaces from its cards (reordering, showing and creating them),
// showing the displayed profile's favorites, recording each profile's history, running the items
// of the rows' and the Spaces' native menus, collapsing and deleting Spaces, and quitting, in the
// real app.
// Usage: `mise run e2e`, which bundles the app and writes a session first.
// Rows are tracked as DOM elements: their titles change as pages load, React keeps the elements.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { connect, launch, pages, quit, REVEAL, SIDEBAR, waitFor } from "./devtools.mjs";

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
    // The last command a check caused, past those the sidebar sends after any state update,
    // such as a favicon cached meanwhile.
    lastCommand: () => e2e.sent.filter((url) => !["/menu/targets/", "/window/colors/"].some((part) => url.includes(part))).at(-1),
  }`);
  await sidebar.eval(`{
    const fetchCommand = fetch;
    // A native menu would wait for a real pick: the checks run its items by id instead.
    window.fetch = (url, init) => {
      e2e.sent.push(String(url));
      // Nor do the checks put anything on the user's clipboard.
      if (String(url).endsWith("/command/link/copy")) return Promise.resolve(new Response("{}"));
      if (["node", "space"].some((kind) => String(url).includes("/command/menu/" + kind + "/"))) {
        return Promise.resolve(new Response("{}"));
      }
      return fetchCommand(url, init);
    };
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

  // The benchmark session pins 5 folders of 10 tabs (crates/arbalete-core/examples/bench-session.rs).
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
    "the favorites show as a grid of 6 tiles in two rows of 3 above the pinned tabs",
    `e2e.tiles = () => [...e2e.page().querySelectorAll(".favorites .favorite")]`,
    `e2e.tiles().length === 6 && e2e.tiles()[0].getAttribute("aria-label") === "Favorite 1"
      && new Set(e2e.tiles().map((tile) => tile.getBoundingClientRect().top)).size === 2
      && e2e.tiles()[2].getBoundingClientRect().top === e2e.tiles()[0].getBoundingClientRect().top
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
    "the footer menu's New Folder adds a folder at the end of the pinned tabs and edits its title",
    `e2e.folders = () => [...e2e.page().querySelectorAll(".pinned > li > .folder")];
     document.querySelector(".new-toggle").click();
     new Promise(requestAnimationFrame).then(() =>
       document.querySelector('[role="menuitem"][aria-label="New Folder"]').click())`,
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
  // Real drags: the mouse pressed, moved in steps and released through DevTools, as Chromium turns
  // it into pointer events. `from` and `to` are expressions for elements, `to` aimed at fractions
  // of its width and height once nothing slides; the press lifts on the center of `from`.
  // `modifiers` 1 holds Option.
  const press = (type, { x, y }, modifiers = 0) =>
    sidebar.call("Input.dispatchMouseEvent", {
      type,
      x,
      y,
      modifiers,
      button: "left",
      buttons: type === "mouseReleased" ? 0 : 1,
      clickCount: 1,
    });
  const point = (element, fx, fy) =>
    sidebar.eval(`(() => {
      const rect = (${element}).getBoundingClientRect();
      return { x: rect.left + rect.width * ${fx}, y: rect.top + rect.height * ${fy} };
    })()`);
  const hold = async (from, to, fx, fy, modifiers = 0) => {
    // Rows still sliding from the last drop are not where they will rest.
    await sidebar.eval(waitFor(`document.getAnimations().length === 0`));
    const [start, end] = [await point(from, 0.5, 0.5), await point(to, fx, fy)];
    await press("mousePressed", start);
    for (let step = 1; step <= 8; step++) {
      const t = step / 8;
      const at = { x: start.x + (end.x - start.x) * t, y: start.y + (end.y - start.y) * t };
      await press("mouseMoved", at, modifiers);
    }
    return end;
  };
  const release = (end, modifiers = 0) => press("mouseReleased", end, modifiers);
  // Unless motion is reduced, some row slides to its new place with a transition.
  const slides = `(matchMedia("(prefers-reduced-motion: reduce)").matches
    || [...e2e.page().querySelectorAll("li[data-id], .folder")].some((row) => row.style.transition.includes("transform")))`;

  await sidebar.eval(`e2e.lifted = e2e.tiles()[1].closest("li"); e2e.liftedId = e2e.lifted.dataset.id; 0`);
  let end = await hold(`e2e.tiles()[1]`, `e2e.tiles()[2]`, 0.5, 0.5);
  await check(
    "a dragged favorite moves with the pointer itself and the others slide to open its slot",
    ``,
    `(() => {
      const rect = e2e.lifted.getBoundingClientRect();
      return e2e.lifted.classList.contains("lifted") && e2e.tiles()[2].closest("li") === e2e.lifted
        && Math.abs(rect.left + rect.width / 2 - ${end.x}) <= 2 && Math.abs(rect.top + rect.height / 2 - ${end.y}) <= 2;
    })()`,
  );
  await release(end);
  await check(
    "dropping the favorite settles it in its new slot",
    ``,
    `e2e.tiles()[2].closest("li").dataset.id === e2e.liftedId && !e2e.lifted.classList.contains("lifted")`,
  );
  const liftedId = Number(await sidebar.eval(`e2e.liftedId`));
  await checkSession("the session saves the favorites' new order", (saved) =>
    saved.nodes[saved.profiles[0].favorites].children[2] === liftedId,
  );

  await sidebar.eval(`e2e.rows()[0].scrollIntoView({ block: "center" }); e2e.dragged = e2e.rows()[4]; e2e.target = e2e.rows()[1]; 0`);
  end = await hold(`e2e.dragged`, `e2e.target`, 0.5, 0.1);
  await check(
    "a dragged row stays in place, dimmed, while its ghost follows and a line marks the slot",
    ``,
    `(() => {
      const line = e2e.indicator().getBoundingClientRect();
      const ghost = document.querySelector(".drag-ghost")?.getBoundingClientRect();
      return e2e.dragged.classList.contains("dragged") && e2e.rows()[4] === e2e.dragged && !!ghost
        && ghost.top <= ${end.y} && ghost.bottom >= ${end.y}
        && Math.abs(line.top + line.height / 2 - e2e.target.getBoundingClientRect().top) <= 1.5 && line.height === 1.5;
    })()`,
  );
  await release(end);
  await check(
    "dropping the row moves it to the line, the rows below sliding",
    ``,
    `e2e.rows()[1] === e2e.dragged && !e2e.dragged.classList.contains("dragged")
      && !document.querySelector(".drag-ghost") && getComputedStyle(e2e.indicator()).display === "none" && ${slides}`,
  );

  await sidebar.eval(`e2e.reading.scrollIntoView({ block: "center" }); e2e.dragged = e2e.rows()[0]; e2e.moved = e2e.dragged.dataset.id; 0`);
  end = await hold(`e2e.dragged`, `e2e.reading`, 0.5, 0.8);
  await check(
    "over the lower half of a folder, the line shows under it, indented",
    ``,
    `(() => {
      const line = e2e.indicator().getBoundingClientRect();
      const folder = e2e.reading.getBoundingClientRect();
      return Math.abs(line.top + line.height / 2 - folder.bottom) <= 1.5 && line.left >= folder.left + 12;
    })()`,
  );
  await release(end);
  await check(
    "dropping it there makes the tab the folder's first",
    ``,
    `e2e.reading.parentElement.querySelector(".folder-tabs > li")?.dataset.id === e2e.moved && !e2e.has(e2e.rows(), e2e.moved)`,
  );

  await sidebar.eval(`e2e.rows()[0].scrollIntoView({ block: "center" }); e2e.dragged = e2e.rows()[1]; e2e.target = e2e.rows()[0];
    e2e.pair = [e2e.target.dataset.id, e2e.dragged.dataset.id]; 0`);
  end = await hold(`e2e.dragged`, `e2e.target`, 0.5, 0.5);
  await check(
    "over the middle of another tab row, the row previews the split, without a line",
    ``,
    `document.querySelectorAll(".split-preview .half").length === 2 && e2e.target.classList.contains("split-target")
      && getComputedStyle(e2e.indicator()).display === "none"`,
  );
  await release(end);
  await check(
    "dropping it there makes a split row of both tabs",
    ``,
    `(() => {
      const split = e2e.page().querySelector(".today > .split-row");
      const halves = [...(split?.querySelectorAll(".half") ?? [])].map((half) => half.dataset.tab);
      return split && split.getBoundingClientRect().height === 41 && halves.join() === e2e.pair.join()
        && !document.querySelector(".split-preview, .split-target");
    })()`,
  );
  await check(
    "the split row's two tab pills keep their fill after the drop",
    ``,
    `[...e2e.page().querySelectorAll(".split-row .half")]
      .every((half) => getComputedStyle(half).backgroundColor !== "rgba(0, 0, 0, 0)")`,
  );
  const pair = (await sidebar.eval(`e2e.pair`)).map(Number);
  await checkSession("the session saves the split", (saved) =>
    Object.values(saved.nodes).some((node) => node.kind === "Split" && node.children.join() === pair.join()),
  );
  await check(
    "clicking a half of the split selects its tab, the row lit and that half the brighter",
    `e2e.page().querySelector(".split-row .half:last-child").click()`,
    `(() => {
      const [first, last] = e2e.page().querySelectorAll(".split-row .half");
      const fill = (element) => getComputedStyle(element).backgroundColor;
      return last.getAttribute("aria-current") === "page" && fill(first) !== fill(last)
        && fill(e2e.page().querySelector(".split-row .split")) !== "rgba(0, 0, 0, 0)";
    })()`,
  );
  await check(
    "closing a half leaves the other tab as a normal row",
    `e2e.send("close/" + e2e.pair[1])`,
    `!e2e.page().querySelector(".split-row") && e2e.rows()[0].dataset.id === e2e.pair[0]`,
  );

  await sidebar.eval(`e2e.rows()[0].scrollIntoView({ block: "center" }); e2e.dragged = e2e.rows()[3];
    e2e.count = e2e.rows().length; e2e.copied = e2e.dragged.textContent; 0`);
  end = await hold(`e2e.dragged`, `e2e.rows()[0]`, 0.5, 0.1);
  await press("mouseMoved", end, 1);
  await check(
    "holding Option while dragging a tab shows a \"+\" badge on its ghost, the tab staying lit",
    ``,
    `!!document.querySelector(".drag-ghost.copy") && getComputedStyle(document.querySelector(".copy-badge")).display === "block"
      && !e2e.dragged.classList.contains("dragged")`,
  );
  await press("mouseMoved", end);
  await check(
    "releasing Option mid-drag takes the badge away",
    ``,
    `!document.querySelector(".drag-ghost.copy") && e2e.dragged.classList.contains("dragged")`,
  );
  await press("mouseMoved", end, 1);
  await release(end, 1);
  await check(
    "dropping it with Option inserts a copy at the line and leaves the tab where it was",
    ``,
    `e2e.rows().length === e2e.count + 1 && e2e.rows()[4] === e2e.dragged && e2e.rows()[0] !== e2e.dragged
      && e2e.rows()[0].textContent === e2e.copied && !document.querySelector(".drag-ghost")`,
  );
  const [copyId, originalId] = await sidebar.eval(`[e2e.rows()[0].dataset.id, e2e.dragged.dataset.id]`);
  await checkSession("the session saves the copy as another tab with the same url", (saved) =>
    copyId !== originalId && saved.nodes[copyId]?.kind.Tab?.url === saved.nodes[originalId].kind.Tab.url,
  );

  await sidebar.eval(`e2e.pair = [e2e.rows()[0].dataset.id, e2e.rows()[1].dataset.id]; 0`);
  await check(
    "a split made from two tabs shows as one row",
    `e2e.send("split/" + e2e.pair[1] + "/" + e2e.pair[0])`,
    `e2e.page().querySelector(".today > .split-row")?.querySelectorAll(".half").length === 2`,
  );
  await sidebar.eval(waitFor(`document.getAnimations().length === 0`));
  const half = await point(`e2e.page().querySelector(".today > .split-row .half:last-child")`, 0.5, 0.5);
  await press("mouseMoved", half);
  await check(
    "hovering a split row without Option shows no separate button",
    ``,
    `[...e2e.page().querySelectorAll(".split-row .separate")].every((button) => getComputedStyle(button).display === "none")`,
  );
  await press("mouseMoved", half, 1);
  await check(
    "holding Option over a split row shows a 20 px separate button in each half",
    ``,
    `[...e2e.page().querySelectorAll(".split-row .separate")].every((button) => {
      const rect = button.getBoundingClientRect();
      return getComputedStyle(button).display === "grid" && rect.width === 20 && rect.height === 20;
    })`,
  );
  const separate = await point(`e2e.page().querySelectorAll(".split-row .separate")[1]`, 0.5, 0.5);
  await press("mouseMoved", separate, 1);
  await press("mousePressed", separate, 1);
  await press("mouseReleased", separate, 1);
  await check(
    "clicking a half's separate button takes its tab out, as the row after the other",
    ``,
    `!e2e.page().querySelector(".split-row") && e2e.rows()[0].dataset.id === e2e.pair[0]
      && e2e.rows()[1].dataset.id === e2e.pair[1]`,
  );
  await press("mouseMoved", { x: separate.x, y: separate.y + 200 });
  await check(
    "moving without Option forgets it",
    ``,
    `!document.querySelector(".sidebar").hasAttribute("data-option")`,
  );
  await sidebar.eval(waitFor(`document.getAnimations().length === 0`));
  // A transition on a row's transform, running: the rows below the folder slide by.
  const sliding = `[...e2e.page().querySelectorAll("li[data-id], .folder")].some((row) =>
    row.getAnimations().some((animation) => animation.transitionProperty === "transform" && animation.playState === "running"))`;
  await check(
    "a folder closing slides the rows below it up while its tabs fade out",
    `e2e.folder = e2e.folders()[0]; e2e.folder.scrollIntoView({ block: "start" });
     e2e.send("toggle/" + e2e.folder.parentElement.dataset.id)`,
    `matchMedia("(prefers-reduced-motion: reduce)").matches
      || (${sliding} && !!document.querySelector(".leaving"))`,
  );
  await sidebar.eval(waitFor(`document.getAnimations().length === 0 && !document.querySelector(".leaving")`));
  await check(
    "a folder opening slides the rows below it down while its tabs fade in",
    `e2e.send("toggle/" + e2e.folder.parentElement.dataset.id)`,
    `matchMedia("(prefers-reduced-motion: reduce)").matches
      || (${sliding} && e2e.folder.parentElement.querySelectorAll(".folder-tabs li").length === 10
        && e2e.folder.parentElement.querySelector(".folder-tabs li").getAnimations().length > 0)`,
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
  // The pointer rests on the toggle, which then hides and shows the sidebar under it.
  const toggle = await point(`document.querySelector('[aria-label="Toggle sidebar"]')`, 0.5, 0.5);
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", ...toggle });
  await check(
    "the hovered toolbar button takes a fill",
    ``,
    `getComputedStyle(document.querySelector('[aria-label="Toggle sidebar"]')).backgroundColor !== "rgba(0, 0, 0, 0)"`,
  );
  await check(
    "hidden and shown again under a resting pointer, the toggle shows no stale fill",
    `e2e.send("sidebar/toggle").then(() => e2e.send("sidebar/toggle"))`,
    `!document.documentElement.hasAttribute("data-undocked") && document.documentElement.hasAttribute("data-stale-hover")
      && getComputedStyle(document.querySelector('[aria-label="Toggle sidebar"]')).backgroundColor === "rgba(0, 0, 0, 0)"`,
  );
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", x: toggle.x + 1, y: toggle.y });
  await check(
    "the pointer moving again shows the hover",
    ``,
    `!document.documentElement.hasAttribute("data-stale-hover")
      && getComputedStyle(document.querySelector('[aria-label="Toggle sidebar"]')).backgroundColor !== "rgba(0, 0, 0, 0)"`,
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
       [...document.querySelectorAll('[role="menuitem"]')].find((item) => item.getAttribute("aria-label") === name).click();
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
  // The app sends a trackpad swipe, from touch to lift, as arbalete:swipe events: `dx` is how
  // far the content would scroll, in points, `time` when, in milliseconds (app/src/swipe.rs).
  await sidebar.eval(`Object.assign(e2e, {
    gesture: (phase, dx, time, y = 300) =>
      dispatchEvent(new CustomEvent("arbalete:swipe", { detail: { phase, dx, y, time } })),
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
    addEventListener("arbalete:state", (event) => { e2e.state = event.detail; e2e.spaces = event.detail.spaces.map((space) => space.id) });
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
    "a swipe before the first Space shows the library on its Spaces, the app keeping the first Space",
    `e2e.swipe(20, 15, 50)`,
    `e2e.library()?.querySelector('[role=tab][aria-selected=true]')?.textContent === "Spaces"
      && e2e.library().querySelectorAll(".space-card").length === e2e.spaces.length && e2e.offset() === 100
      && e2e.state.space === e2e.spaces[0] && !e2e.page()
      && e2e.lastCommand().endsWith("/library/show")`,
  );
  await check(
    "its Recently Closed section lists the closed tab",
    `e2e.section = (name) => [...e2e.library().querySelectorAll("[role=tab]")].find((tab) => tab.textContent === name);
     e2e.section("Recently Closed").click()`,
    `e2e.library().querySelector(".closed-tab .title")?.textContent === e2e.closedText
      && e2e.library().querySelector(".library-search").placeholder === "Search Recently Closed…"`,
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
    `e2e.closedCount = e2e.state.closed.length; e2e.library().querySelector(".closed-tab").click()`,
    `e2e.settled(0) && !e2e.library() && e2e.rows().length === e2e.count
      && e2e.state.closed.length === e2e.closedCount - 1 && innerWidth === 260`,
  );
  await check("the card gets its place back beside the sidebar", ``, `innerWidth === ${width - 260}`, content);
  await check(
    "the footer's Library button slides to the library on the section picked last, the app keeping the first Space",
    `document.querySelector('.switcher [aria-label="Library"]').click()`,
    `e2e.library() && e2e.offset() === 100 && e2e.state.space === e2e.spaces[0]
      && e2e.library().querySelector("[role=tab][aria-selected=true]").textContent === "Recently Closed"`,
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
  await check(
    "a right-click on a row asks for its menu, the page's menu left out",
    `e2e.send("space/number/1");
     e2e.rightClick = (element) => !element.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
     e2e.row = (id) => e2e.page()?.querySelector('li[data-id="' + id + '"]');
     e2e.choose = (id, item) => e2e.send("menu/choose/node/" + id + "/" + item);
     // The url of a tab or a favorite of the displayed Space.
     e2e.urlOf = (id) => e2e.state.pages.find((page) => page.space === e2e.state.space)
       .today.concat(e2e.state.pages.find((page) => page.space === e2e.state.space).favorites)
       .find((tab) => String(tab.id) === id)?.url`,
    `e2e.settled(0) && (e2e.menuTab = e2e.rows()[0].dataset.id)
      && e2e.rightClick(e2e.rows()[0].querySelector(".title"))
      && e2e.lastCommand().endsWith("/command/menu/node/" + e2e.menuTab)`,
  );
  await check(
    "a right-click elsewhere in the sidebar shows no menu at all",
    `e2e.sentCount = e2e.sent.length;
     e2e.prevented = e2e.rightClick(document.querySelector(".toolbar")) && e2e.rightClick(document.querySelector(".switcher"))`,
    `e2e.prevented && e2e.sent.length === e2e.sentCount`,
  );
  await check(
    "the sidebar tells the app where its rows and Space header are, for real right-clicks",
    ``,
    `(() => {
      const url = e2e.sent.filter((url) => url.includes("/menu/targets/docked/")).at(-1);
      const targets = JSON.parse(decodeURIComponent(url.split("/menu/targets/docked/")[1]));
      const at = (element, target) => {
        const rect = element.getBoundingClientRect();
        return targets.some((t) => JSON.stringify(t.target) === JSON.stringify(target)
          && Math.abs(t.y - rect.top) < 1 && Math.abs(t.height - rect.height) < 1);
      };
      const folder = e2e.page().querySelector(".pinned > .folder-item");
      return at(e2e.page().querySelector(".space-header"), { space: e2e.spaces[0] })
        && at(folder.querySelector(":scope > .folder"), { node: Number(folder.dataset.id) })
        && at(e2e.tiles()[0], { node: Number(e2e.tiles()[0].closest("li").dataset.id) });
    })()`,
  );
  await check(
    "the tab menu's Pin Tab pins it",
    `e2e.choose(e2e.menuTab, "pin")`,
    `e2e.row(e2e.menuTab)?.closest(".pinned") && e2e.above(e2e.row(e2e.menuTab))`,
  );
  await check(
    "Unpin Tab puts it back in today's tabs",
    `e2e.choose(e2e.menuTab, "pin")`,
    `!!e2e.row(e2e.menuTab)?.closest(".today")`,
  );
  await check(
    "Move To another Space takes the tab away, the displayed Space kept",
    `e2e.choose(e2e.menuTab, "move/" + e2e.spaces[1])`,
    `!e2e.row(e2e.menuTab) && e2e.settled(0)`,
  );
  const moved = Number(await sidebar.eval(`e2e.menuTab`));
  await checkSession("the session saves the moved tab last in that Space's tabs", (saved) =>
    saved.nodes[saved.spaces[1].today].children.at(-1) === moved,
  );
  await check(
    "moving the selected tab takes the sidebar along to its new Space",
    `e2e.menuTab = e2e.rows()[0].dataset.id;
     e2e.send("select/" + e2e.menuTab).then(() => e2e.choose(e2e.menuTab, "move/" + e2e.spaces[1]))`,
    `e2e.settled(1) && e2e.currentId() === e2e.menuTab && e2e.rows().at(-1) === e2e.current()`,
  );
  await check(
    "Close Tab closes it",
    `e2e.menuUrl = e2e.urlOf(e2e.menuTab); e2e.choose(e2e.menuTab, "close")`,
    `!e2e.row(e2e.menuTab) && e2e.state.closed[0].url === e2e.menuUrl`,
  );
  await check("the first Space shows again", `e2e.send("space/number/1")`, `e2e.settled(0)`);
  await check(
    "the tab menu's New Space slides to the page creating one",
    `e2e.menuTab = e2e.rows()[0].dataset.id; e2e.choose(e2e.menuTab, "new-space")`,
    `e2e.newPage() && document.activeElement === e2e.newField("name")`,
  );
  await type("Moved");
  await pressEnter();
  await check(
    "creating it moves the tab into the new Space, selected",
    ``,
    `e2e.last().getAttribute("aria-label") === "Moved" && e2e.last().getAttribute("aria-current") === "true"
      && e2e.rows().length === 1 && e2e.currentId() === e2e.menuTab`,
  );
  await check(
    "the first Space shows once more",
    `e2e.send("space/number/1");
     e2e.folderRow = (title) => [...e2e.page().querySelectorAll(".folder-item")]
       .find((row) => row.querySelector(":scope > .folder").textContent === title)`,
    `e2e.settled(0)`,
  );
  await check(
    "the folder menu's Rename Folder edits its name",
    `e2e.choose(e2e.folderRow("Folder 3").dataset.id, "rename")`,
    `document.activeElement === e2e.page().querySelector('[aria-label="Folder name"]')`,
  );
  await type("Reading");
  await pressEnter();
  await check("the folder takes its new name", ``, `!!e2e.folderRow("Reading")`);
  await check(
    "New Folder creates one inside it, its name being edited",
    `e2e.choose(e2e.folderRow("Reading").dataset.id, "new-folder")`,
    `document.activeElement === e2e.folderRow("Reading").querySelector('.folder-tabs [aria-label="Folder name"]')`,
  );
  await pressEnter();
  await check(
    "the new folder sits inside, empty",
    ``,
    `e2e.folderRow("Reading").querySelector(".folder-tabs .folder-item .folder")?.textContent === "New Folder"
      && e2e.folderRow("Reading").querySelectorAll(".folder-tabs .folder-item .tab-row").length === 0`,
  );
  await check(
    "Close Tabs in Folder keeps a pinned folder's tabs, the selected one deselected",
    `e2e.folder4 = e2e.folderRow("Folder 4").dataset.id;
     e2e.send("select/" + e2e.folderRow("Folder 4").querySelector(".tab-row").dataset.id)
       .then(() => e2e.choose(e2e.folder4, "close-tabs"))`,
    `e2e.folderRow("Folder 4").querySelectorAll(".tab-row").length === 10
      && e2e.current() && !e2e.folderRow("Folder 4").contains(e2e.current())`,
  );
  await check(
    "Close Tabs in Folder closes the tabs of a folder among today's",
    `e2e.folder5 = e2e.folderRow("Folder 5").dataset.id;
     e2e.send("move/" + e2e.folder5 + "/" + e2e.page().querySelector(".today").dataset.parent + "/0")
       .then(() => e2e.choose(e2e.folder5, "close-tabs"))`,
    `e2e.folderRow("Folder 5")?.closest(".today") && e2e.folderRow("Folder 5").querySelectorAll(".tab-row").length === 0`,
  );
  await check(
    "the favorite menu's Remove from Favorites removes it, which can be reopened",
    `e2e.tileCount = e2e.tiles().length;
     e2e.tile = e2e.tiles()[0].closest("li");
     e2e.tileUrl = e2e.urlOf(e2e.tile.dataset.id);
     e2e.prevented = e2e.rightClick(e2e.tile.querySelector(".favorite"));
     e2e.choose(e2e.tile.dataset.id, "remove")`,
    `e2e.prevented && e2e.sent.some((url) => url.endsWith("/menu/node/" + e2e.tile.dataset.id))
      && e2e.tiles().length === e2e.tileCount - 1 && !e2e.tile.isConnected
      && e2e.state.closed[0].url === e2e.tileUrl`,
  );
  await check(
    "a right-click on the Space's header asks for the Space's menu",
    `e2e.header = () => e2e.page().querySelector(".space-toggle");
     e2e.chooseSpace = (item) => e2e.send("menu/choose/space/" + e2e.spaces[0] + "/" + item);
     e2e.prevented = e2e.rightClick(e2e.header())`,
    `e2e.prevented && e2e.lastCommand().endsWith("/command/menu/space/" + e2e.spaces[0])`,
  );
  await check(
    "the header's options button asks for it too",
    `e2e.page().querySelector(".space-more").click()`,
    `e2e.sent.filter((url) => url.endsWith("/command/menu/space/" + e2e.spaces[0])).length === 2`,
  );
  await check(
    "a click on the header collapses the Space, leaving only New tab",
    `e2e.header().click()`,
    `e2e.header().getAttribute("aria-expanded") === "false" && !e2e.page().querySelector(".pinned, .today, .divider")
      && [...e2e.page().querySelectorAll(".tab")].map((tab) => tab.textContent).join() === "+New tab"`,
  );
  await checkSession("the session keeps the Space collapsed", (saved) => saved.spaces[0].collapsed === true);
  await check(
    "another click expands it again",
    `e2e.header().click()`,
    `e2e.header().getAttribute("aria-expanded") === "true" && e2e.rows().length > 0`,
  );
  await check(
    "the Space menu's Change Space Icon opens the editor on the emoji",
    `e2e.chooseSpace("icon")`,
    `document.activeElement === e2e.spaceField("emoji")`,
  );
  await check(
    "Rename Space moves to the name",
    `e2e.chooseSpace("rename")`,
    `document.activeElement === e2e.spaceField("name")`,
  );
  await check(
    "Edit Theme Color moves to the colors",
    `e2e.chooseSpace("color")`,
    `document.activeElement?.matches(".space-editor .swatch[aria-pressed=true]")`,
  );
  await check(
    "New Folder adds a folder to the Space's pinned tabs, its name being edited",
    `e2e.spaceIcons()[0].click(); e2e.folders = e2e.page().querySelectorAll(".pinned > .folder-item").length;
     e2e.chooseSpace("new-folder")`,
    `e2e.page().querySelectorAll(".pinned > .folder-item").length === e2e.folders + 1
      && document.activeElement === e2e.page().querySelector('.pinned > .folder-item:last-child [aria-label="Folder name"]')`,
  );
  await pressEnter();
  await check(
    "Manage Spaces slides to the library's Spaces",
    `e2e.chooseSpace("manage")`,
    `e2e.library() && e2e.offset() === 100
      && e2e.library().querySelector("[role=tab][aria-selected=true]").textContent === "Spaces"`,
  );
  await check("the first Space's dot slides back from the library", `e2e.spaceIcons()[0].click()`, `e2e.settled(0)`);
  const lastSpace = await sidebar.eval(`e2e.spaces.length - 1`);
  await check(
    "deleting the displayed last Space displays the one before it",
    `e2e.deleted = e2e.spaces.at(-1);
     e2e.send("space/number/${lastSpace + 1}").then(() => e2e.send("space/delete/" + e2e.deleted))`,
    `e2e.spaces.length === ${lastSpace} && !e2e.spaces.includes(e2e.deleted) && e2e.settled(${lastSpace - 1})`,
  );
  const deleted = Number(await sidebar.eval(`e2e.deleted`));
  await checkSession("the session forgets the deleted Space", (saved) =>
    saved.spaces.every((space) => space.id !== deleted),
  );
  await check("the first Space shows for its profile", `e2e.spaceIcons()[0].click()`, `e2e.settled(0)`);
  const workFavorites = readSession().nodes[favoritesOf(otherProfileSpace)].children.length;
  await check(
    "Set Profile attaches the Space to another profile, whose favorites it shows",
    `e2e.chooseSpace("profile/${otherProfileSpace.profile}")`,
    `e2e.tiles().length === ${workFavorites}`,
  );
  await checkSession("the session saves the Space's new profile", (saved) =>
    saved.spaces[0].profile === otherProfileSpace.profile,
  );
  await check(
    "New Profile attaches it to a new profile, whose name is being edited",
    `e2e.chooseSpace("new-profile")`,
    `e2e.tiles().length === 0 && document.activeElement?.getAttribute("aria-label") === "Profile name"
      && document.activeElement.value === "New Profile"`,
  );
  await checkSession("the session saves the Space with its new profile", (saved) =>
    saved.profiles.find((profile) => profile.id === saved.spaces[0].profile)?.name === "New Profile",
  );
  await pressEnter();
  await check(
    "Escape closes the profiles, which covered the tabs",
    `dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }))`,
    `!document.querySelector(".profiles")`,
  );
  const newProfile = () => {
    const saved = readSession();
    return saved.profiles.find((profile) => profile.id === saved.spaces[0].profile);
  };
  await check(
    "empty favorites show a dashed box with a star, a title, a subtitle and a ×",
    ``,
    `(() => {
      const hint = e2e.page().querySelector(".favorites-empty");
      return !!hint && !!hint.querySelector("svg") && hint.querySelector("strong").textContent === "Drag tabs here to keep them close"
        && hint.querySelector("span").textContent.length > 0 && getComputedStyle(hint).borderStyle === "dashed"
        && !!hint.querySelector('[aria-label="Hide this hint"]');
    })()`,
  );
  await check(
    "the hint's × hides it",
    `e2e.page().querySelector('.favorites-empty [aria-label="Hide this hint"]').click()`,
    `!e2e.page().querySelector(".favorites-empty") && e2e.page().querySelector(".favorites").children.length === 0`,
  );
  await checkSession("the session keeps the hint hidden for the profile", () => newProfile().favorites_hint_hidden === true);
  await check(
    "the footer menu lists New Space, New Folder, New Tab and Profiles with icons, shortcuts and separators",
    `document.querySelector(".new-toggle").click()`,
    `(() => {
      const menu = document.querySelector('.menu[role="menu"]');
      const items = [...(menu?.querySelectorAll('[role="menuitem"]') ?? [])];
      return items.map((item) => item.getAttribute("aria-label")).join() === "New Space,New Folder,New Tab,Profiles…"
        && items.every((item) => item.querySelector("svg"))
        && items[2].querySelector("kbd")?.textContent === "⌘T"
        && menu.querySelectorAll("hr").length === 2
        && document.querySelector(".new-toggle").getAttribute("aria-expanded") === "true";
    })()`,
  );
  await check(
    "the menu's New Folder adds a folder to the pinned tabs, its name being edited",
    `e2e.folders = e2e.page().querySelectorAll(".pinned > .folder-item").length;
     document.querySelector('[role="menuitem"][aria-label="New Folder"]').click()`,
    `e2e.page().querySelectorAll(".pinned > .folder-item").length === e2e.folders + 1
      && !document.querySelector(".menu") && document.activeElement?.getAttribute("aria-label") === "Folder name"`,
  );
  await pressEnter();
  await check(
    "the menu's New Tab opens the overlay to type an address",
    `e2e.menu("New Tab")`,
    `e2e.lastCommand().endsWith("/command/overlay/show") && !document.querySelector(".menu")`,
  );
  await sidebar.eval(`e2e.send("overlay/hide")`);
  await check(
    "13 new tabs open",
    `(async () => {
      e2e.count = e2e.rows().length;
      for (let i = 0; i < 13; i++) await e2e.send("open/" + encodeURIComponent("about:blank#" + i));
    })()`,
    `e2e.rows().length === e2e.count + 13`,
  );
  await check(
    "12 of them become favorites, filling 3 rows of 4 tiles of one width, the hint back for later",
    `(async () => {
      const root = e2e.page().querySelector(".favorites").dataset.parent;
      for (const row of e2e.rows().slice(-13, -1)) await e2e.send("move/" + row.dataset.id + "/" + root + "/99");
    })()`,
    `(() => {
      const tiles = e2e.tiles().map((tile) => tile.getBoundingClientRect());
      return tiles.length === 12 && new Set(tiles.map((tile) => tile.top)).size === 3
        && new Set(tiles.map((tile) => tile.width)).size === 1
        && Math.abs(tiles[3].right - e2e.page().querySelector(".favorites").getBoundingClientRect().right) < 1;
    })()`,
  );
  await checkSession("adding a favorite brings the hint back for the profile", () => newProfile().favorites_hint_hidden === false);
  await sidebar.eval(`e2e.send("move/" + e2e.rows().at(-1).dataset.id + "/" + e2e.page().querySelector(".favorites").dataset.parent + "/0")`);
  await sleep(200);
  await check("the app refuses a 13th favorite", ``, `e2e.tiles().length === 12 && e2e.rows().length === e2e.count + 1`);
  await sidebar.eval(`e2e.rows().at(-1).scrollIntoView({ block: "nearest" })`);
  end = await hold(`e2e.rows().at(-1)`, `e2e.tiles()[5]`, 0.5, 0.5);
  await check(
    "a tab dragged over full favorites opens no slot and says they are full",
    ``,
    `e2e.page().querySelector(".favorites-full")?.textContent === "Favorites are full" && !e2e.page().querySelector(".favorite-slot")`,
  );
  await release(end);
  await check(
    "dropping it there keeps the tab where it was, the hint fading after a moment",
    ``,
    `e2e.tiles().length === 12 && e2e.rows().length === e2e.count + 1 && !e2e.page().querySelector(".favorites-full")`,
  );
  // The pointer rests on the Library button, then leaves it.
  const library = await point(`document.querySelector('.switcher [aria-label="Library"]')`, 0.5, 0.5);
  const away = await point(`document.querySelector(".url")`, 0.3, 0.5);
  await sidebar.eval(`e2e.send("close/" + e2e.rows().at(-1).dataset.id)`);
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", ...away });
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", ...library });
  await check(
    "resting on the Library button shows the closed tabs above the footer, with how long ago",
    ``,
    `(() => {
      const items = [...document.querySelectorAll(".library-popover li button")];
      const popover = document.querySelector(".library-popover")?.getBoundingClientRect();
      return items.length === e2e.state.closed.length && items[0].querySelector(".when").textContent === "Just now"
        && popover.bottom <= document.querySelector(".switcher").getBoundingClientRect().top + 1;
    })()`,
  );
  await check(
    "clicking one reopens it in the displayed Space, the popover gone",
    `e2e.count = e2e.rows().length; document.querySelector(".library-popover li button").click()`,
    `e2e.rows().length === e2e.count + 1 && e2e.urlOf(e2e.rows().at(-1).dataset.id) === "about:blank#12"
      && !document.querySelector(".library-popover")`,
  );
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", ...away });
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", ...library });
  await check("resting on it again shows the popover", ``, `!!document.querySelector(".library-popover")`);
  await sidebar.call("Input.dispatchMouseEvent", { type: "mouseMoved", ...away });
  await check("leaving it hides the popover", ``, `!document.querySelector(".library-popover")`);
  await check(
    "the url bar's copy button copies the link and says so",
    `document.querySelector(".copy-link").click()`,
    `e2e.lastCommand().endsWith("/command/link/copy") && document.querySelector(".copy-link").textContent === "Copied"`,
  );
  await check(
    "the × of a pinned tab its Space does not show removes it, to the closed tabs",
    `e2e.unshown = [...e2e.page().querySelectorAll(".pinned .tab-row")].find((row) => !row.querySelector("[aria-current]"));
     e2e.unshownUrl = e2e.state.pages.find((page) => page.space === e2e.state.space).pinned
       .flatMap((item) => item.children ?? [item]).find((tab) => String(tab.id) === e2e.unshown.dataset.id)?.url;
     e2e.unshown.querySelector(".close").click()`,
    `!e2e.unshown.isConnected && e2e.state.closed[0].url === e2e.unshownUrl`,
  );
  await check(
    "the divider's Clear closes every tab of today, which the closed tabs list",
    `e2e.todayCount = e2e.rows().length; e2e.page().querySelector(".clear-today").click()`,
    `e2e.rows().length === 0 && !e2e.page().querySelector(".clear-today")
      && e2e.state.closed.length === Math.min(10, e2e.state.closed.length) && e2e.state.closed.length > 0
      && !e2e.page().querySelector(".today .tab[aria-current]")`,
  );
  // The library: its sections, searching them, and managing Spaces from their cards.
  await check(
    "the Library button opens the library, wider, on the section picked last",
    `document.querySelector('.switcher [aria-label="Library"]').click()`,
    `e2e.library() && e2e.offset() === 100 && innerWidth === 440
      && e2e.library().querySelector("[role=tab][aria-selected=true]").textContent === "Spaces"`,
  );
  await check(
    "Recently Closed lists the closed tabs under a field searching them",
    `e2e.section("Recently Closed").click()`,
    `e2e.library().querySelectorAll(".closed-tab").length === e2e.state.closed.length
      && e2e.library().querySelector(".library-search").placeholder === "Search Recently Closed…"`,
  );
  const query = await sidebar.eval(`(() => {
    const tab = e2e.state.closed.at(-1);
    e2e.query = (tab.title || tab.url).slice(0, 8);
    e2e.library().querySelector(".library-search").focus();
    return e2e.query;
  })()`);
  await type(query);
  await check(
    "typing in the field keeps the closed tabs holding the text",
    ``,
    `(() => {
      const wanted = e2e.query.toLowerCase();
      const found = e2e.state.closed.filter((tab) => (tab.title + " " + tab.url).toLowerCase().includes(wanted));
      const rows = [...e2e.library().querySelectorAll(".closed-tab")];
      return found.length > 0 && rows.length === found.length
        && rows.every((row) => (row.textContent + row.title).toLowerCase().includes(wanted));
    })()`,
  );
  await check(
    "the Spaces section shows a card per Space, the field emptied",
    `e2e.section("Spaces").click()`,
    `e2e.library().querySelectorAll(".space-card").length === e2e.spaces.length
      && e2e.library().querySelector(".library-search").value === ""
      && e2e.library().querySelector(".library-search").placeholder === "Search Spaces…"`,
  );
  await sidebar.eval(`e2e.cards = () => [...e2e.library().querySelectorAll(".space-card")];
    e2e.order = e2e.spaces.slice(); e2e.shownSpace = e2e.state.space; 0`);
  end = await hold(`e2e.cards()[1].querySelector(".space-card-handle")`, `e2e.cards()[0]`, 0.25, 0.5);
  await check(
    "a card dragged by its handle stays dimmed while its copy follows the pointer and a line shows where it lands",
    ``,
    `(() => {
      const ghost = document.querySelector(".card-ghost")?.getBoundingClientRect();
      const line = document.querySelector(".space-drop-line");
      const first = e2e.cards()[0].getBoundingClientRect();
      return e2e.cards()[1].classList.contains("dragged") && ghost.width === first.width
        && ghost.left < ${end.x} && ghost.right > ${end.x}
        && getComputedStyle(line).display === "block" && line.getBoundingClientRect().left < first.left;
    })()`,
  );
  await release(end);
  await check(
    "dropping it before the first card moves its Space first, the library staying in view",
    ``,
    `e2e.spaces.join() === [e2e.order[1], e2e.order[0], ...e2e.order.slice(2)].join()
      && e2e.cards()[0].dataset.id === String(e2e.order[1]) && !document.querySelector(".card-ghost, .dragged")
      && e2e.library() && e2e.offset() === 100 && e2e.state.space === e2e.shownSpace`,
  );
  const order = await sidebar.eval(`e2e.spaces.join()`);
  await checkSession("the session saves the Spaces' new order", (saved) =>
    saved.spaces.map((space) => space.id).join() === order,
  );
  await check(
    "the profile pill's menu moves a card's Space to another profile",
    `e2e.card = e2e.cards()[0];
     e2e.otherProfile = e2e.state.profiles.find((profile) => profile.id !== e2e.state.spaces[0].profile);
     e2e.send("menu/choose/space-profile/" + e2e.spaces[0] + "/profile/" + e2e.otherProfile.id)`,
    `e2e.state.spaces[0].profile === e2e.otherProfile.id
      && e2e.card.querySelector(".space-card-profile").textContent === e2e.otherProfile.name`,
  );
  await check(
    "clicking a card shows its Space and leaves the library",
    `e2e.cards()[2].querySelector(".space-card-pinned").click()`,
    `e2e.settled(2) && !e2e.library() && e2e.state.space === e2e.spaces[2]`,
  );
  await check(
    "the Library button opens it again",
    `document.querySelector('.switcher [aria-label="Library"]').click()`,
    `e2e.library() && e2e.offset() === 100`,
  );
  await check(
    "the + after the cards slides to the page creating a Space, its name field focused",
    `e2e.spaceCount = e2e.spaces.length; e2e.library().querySelector(".space-add button").click()`,
    `e2e.newPage() && e2e.offset() === -e2e.spaceCount * 100 && document.activeElement === e2e.newField("name")`,
  );
  await type("From the library");
  await pressEnter();
  await check(
    "creating it there adds a Space after the others and shows it",
    ``,
    `e2e.spaces.length === e2e.spaceCount + 1 && e2e.state.spaces.at(-1).title === "From the library"
      && e2e.settled(e2e.spaceCount)`,
  );
  await quit(app);
  console.log("ok - quitting once closes every browser and exits the app");
} finally {
  app.kill();
}
