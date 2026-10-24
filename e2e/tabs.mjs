// Checks the sidebar sections, selecting, closing, reopening and pinning tabs and favorites, and
// creating, renaming and collapsing folders, and dragging tabs between sections, in the real app.
// Usage: `mise run e2e`, which bundles the app and writes a session first.
// Rows are tracked as DOM elements: their titles change as pages load, React keeps the elements.

import { connect, launch, SIDEBAR, waitFor } from "./devtools.mjs";

const dataDir = process.argv[2];
if (!dataDir) throw new Error("usage: node e2e/tabs.mjs <data dir>");

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
    has: (list, id) => [...list].some((element) => element.closest("li").dataset.id === id),
  }`);

  async function check(description, action, condition) {
    await sidebar.eval(action);
    try {
      await sidebar.eval(waitFor(condition));
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
    "scrolling the tabs keeps the url bar in place",
    `e2e.top = document.querySelector(".url").getBoundingClientRect().top; document.querySelector(".scroll").scrollTop = 1000`,
    `document.querySelector(".scroll").scrollTop > 0 && document.scrollingElement.scrollTop === 0
      && document.querySelector(".url").getBoundingClientRect().top === e2e.top`,
  );
} finally {
  app.kill();
}
