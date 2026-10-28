import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { App, COPIED_MS, DOUBLE_CLICK_MS, domain } from "./App";
import { COLORS, type Item, type Page, type Space, type Tab, windowColors } from "./bridge";
import { FULL_HINT_MS, favoriteColumns, MAX_FAVORITES } from "./drag";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const today: Item[] = [
  { id: 3, url: "https://example.com/", title: "Example", favicon: null },
  { id: 4, url: "https://example.org/", title: "Other", favicon: null, muted: true },
  {
    id: 20,
    split: [
      { id: 21, url: "https://example.com/left", title: "Left", favicon: null },
      { id: 22, url: "https://example.com/right", title: "Right", favicon: null },
    ],
  },
];

const pinned: Item[] = [
  { id: 5, url: "https://example.net/", title: "Pinned", favicon: null },
  {
    id: 6,
    title: "Docs",
    expanded: true,
    children: [{ id: 7, url: "https://example.net/docs", title: "Manual", favicon: null }],
  },
  {
    id: 8,
    title: "Archive",
    expanded: false,
    children: [{ id: 9, url: "https://example.net/old", title: "Old", favicon: null }],
  },
];

const favorites: Tab[] = [
  {
    id: 10,
    url: "https://example.com/mail",
    title: "Mail",
    favicon: "https://example.com/mail.ico",
  },
  { id: 11, url: "https://example.com/calendar", title: "", favicon: null },
];

const spaces: Space[] = [
  { id: 13, title: "Home", emoji: null, color: "blue", profile: 1, pinned: [] },
  { id: 14, title: "work", emoji: null, color: "green", profile: 1, pinned: [] },
  { id: 15, title: "Travel", emoji: null, color: "red", profile: 1, pinned: [] },
  { id: 16, title: "Books", emoji: null, color: "gray", profile: 1, pinned: [] },
];

// The first Space shows the tabs above, the others none. The app sends the pages of the displayed
// Space and its neighbors.
function receive(
  page: Partial<Page>,
  active = 3,
  favoriteTabs = favorites,
  displayed = 13,
  collapsed = false,
  hiddenHint = false,
  toolbar = false,
) {
  const empty = { favorites: [], pinned: [], today: [], selected: null, collapsed: false };
  const favoritesHint = !hiddenHint;
  const shown = spaces.findIndex(({ id }) => id === displayed);
  const detail = {
    page: { progress: 1, loading: false, canGoBack: true, canGoForward: true, ...page },
    space: displayed,
    spaces,
    pages: spaces
      .map(({ id }, index) => ({
        space: id,
        roots: { favorites: 1, pinned: 2 + index * 100, today: 12 + index * 100 },
        favoritesHint,
        ...(index === 0
          ? { favorites: favoriteTabs, pinned, today, selected: active, collapsed }
          : { ...empty }),
      }))
      .filter((_, index) => Math.abs(index - shown) <= 1),
    profiles: [{ id: 1, name: "Default", color: "purple" }],
    closed: [{ url: "https://example.com/gone", title: "Gone", favicon: null, closedAt: 0 }],
    toolbar,
  };
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:state", { detail }));
  });
}

// The Space icons at the bottom, whose names are the Spaces' like their pages' headers.
const switcher = () => within(screen.getByRole("list", { name: "Spaces" }));

// The displayed Space's page, apart from its neighbors.
function shown() {
  const page = document.querySelector<HTMLElement>(".space-page:not([inert])");
  if (!page) throw new Error("no displayed page");
  return within(page);
}

test("asks the app for its state", () => {
  render(<App />);
  expect(fetch).toHaveBeenCalledWith("/command/ready", { method: "POST" });
});

test("sends the toolbar's commands to the app", () => {
  render(<App />);
  receive({});
  for (const [label, command] of [
    ["Toggle sidebar", "sidebar/toggle"],
    ["Back", "back"],
    ["Forward", "forward"],
    ["Reload", "reload"],
  ]) {
    fireEvent.click(screen.getByRole("button", { name: label }));
    expect(fetch).toHaveBeenLastCalledWith(`/command/${command}`, { method: "POST" });
  }
});

test("shows the active tab and the loading progress", () => {
  render(<App />);
  receive({ loading: true, progress: 0.4, canGoBack: false });
  const url = screen.getByText("example.com");
  expect(url.closest(".url")?.getAttribute("title")).toBe("https://example.com/");
  expect(screen.getByRole("progressbar", { name: "Loading" })).toBeDefined();
  expect(screen.getByRole("button", { name: "Back" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "Forward" })).toHaveProperty("disabled", false);
  expect(screen.getByRole("button", { name: "Example" }).getAttribute("aria-current")).toBe("page");
});

test("in toolbar mode, the sidebar leaves the navigation buttons and the address to the bar", () => {
  render(<App />);
  receive({}, 3, favorites, 13, false, false, true);
  expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Edit address" })).toBeNull();
  // The sidebar toggle stays.
  expect(screen.getByRole("button", { name: "Toggle sidebar" })).toBeDefined();
  receive({});
  expect(screen.getByRole("button", { name: "Back" })).toBeDefined();
});

test("the url bar's site controls ask the app for the site's menu", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Site controls" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/menu/site", { method: "POST" });
});

test("a click on the url bar edits the address in the Command Bar", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Edit address" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/overlay/edit", { method: "POST" });
});

test("shows a page's domain in the url bar", () => {
  expect(domain("https://www.wikipedia.org/wiki/Arrow")).toBe("wikipedia.org");
  expect(domain("http://docs.example.com:8080/guide?q=1")).toBe("docs.example.com");
  expect(domain("about:blank")).toBe("about:blank");
  expect(domain("not a url")).toBe("not a url");
});

test("the url bar's copy button copies the link and says so for a moment", () => {
  vi.useFakeTimers();
  render(<App />);
  receive({});
  const copy = screen.getByRole("button", { name: "Copy link" });
  fireEvent.click(copy);
  expect(fetch).toHaveBeenLastCalledWith("/command/link/copy", { method: "POST" });
  expect(within(copy).getByRole("status").textContent).toBe("Copied");
  act(() => vi.advanceTimersByTime(COPIED_MS));
  expect(within(copy).queryByRole("status")).toBeNull();
  vi.useRealTimers();
});

test("keeps the url bar without a selected tab", () => {
  render(<App />);
  receive({ loading: true }, 99);
  const url = document.querySelector(".url");
  expect(url?.textContent).toBe("");
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.queryByRole("button", { name: "Copy link" })).toBeNull();
});

test("selects a tab", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Other" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/select/4", { method: "POST" });
});

test("closes a tab", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Close Other" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/close/4", { method: "POST" });
});

test("marks the selected tab's close button as a dash, the others as a cross", () => {
  render(<App />);
  receive({});
  const close = (name: string) => screen.getByRole("button", { name: `Close ${name}` });
  expect(close("Example").textContent).toBe("—");
  expect(close("Other").textContent).toBe("×");
  fireEvent.click(close("Example"));
  expect(fetch).toHaveBeenLastCalledWith("/command/close/3", { method: "POST" });
});

test("shows the displayed Space's name above its pinned tabs", () => {
  render(<App />);
  receive({});
  const header = shown().getByRole("heading", { name: /^Home/ });
  const pinned = shown().getByRole("button", { name: "Pinned" });
  expect(header.compareDocumentPosition(pinned) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

test("shows a Space's emoji in its header", () => {
  render(<App />);
  spaces[0] = { ...spaces[0], emoji: "🏠" };
  try {
    receive({});
    expect(shown().getByRole("button", { name: "Home" }).textContent).toBe("🏠Home");
  } finally {
    spaces[0] = { ...spaces[0], emoji: null };
  }
});

test("collapses the Space with a click on its header", () => {
  render(<App />);
  receive({});
  const header = shown().getByRole("button", { name: "Home" });
  expect(header.getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(header);
  expect(fetch).toHaveBeenLastCalledWith("/command/space/toggle/13", { method: "POST" });
});

test("shows only the header and New tab of a collapsed Space", () => {
  render(<App />);
  receive({}, 3, favorites, 13, true);
  const page = shown();
  expect(page.getByRole("button", { name: "Home" }).getAttribute("aria-expanded")).toBe("false");
  expect(page.getByText("New tab")).toBeDefined();
  for (const name of ["Pinned", "Docs", "Example", "↓ Clear"]) {
    expect(page.queryByRole("button", { name })).toBeNull();
  }
  expect(page.getByRole("button", { name: "Mail" })).toBeDefined();
});

test("the header's options button and a right-click on it ask for the Space's menu", () => {
  render(<App />);
  receive({});
  fireEvent.click(shown().getByRole("button", { name: "Home options" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/menu/space/13", { method: "POST" });
  fetch.mockClear();
  expect(fireEvent.contextMenu(shown().getByRole("button", { name: "Home" }))).toBe(false);
  expect(fetch).toHaveBeenCalledExactlyOnceWith("/command/menu/space/13", { method: "POST" });
});

test("shows pinned tabs and folders above the divider and today's tabs", () => {
  render(<App />);
  receive({});
  const divider = shown().getByRole("separator");
  const above = (element: HTMLElement) =>
    Boolean(element.compareDocumentPosition(divider) & Node.DOCUMENT_POSITION_FOLLOWING);
  expect(above(screen.getByRole("button", { name: "Pinned" }))).toBe(true);
  expect(above(screen.getByText("Docs"))).toBe(true);
  expect(above(screen.getByRole("button", { name: "Manual" }))).toBe(true);
  expect(above(screen.getByRole("button", { name: "Example" }))).toBe(false);
});

test("hides the tabs of collapsed folders", () => {
  render(<App />);
  receive({});
  expect(screen.getByText("Archive")).toBeDefined();
  expect(screen.queryByRole("button", { name: "Old" })).toBeNull();
});

test("selects and closes a tab inside a folder", () => {
  render(<App />);
  receive({}, 7);
  expect(document.querySelector(".url")?.getAttribute("title")).toBe("https://example.net/docs");
  expect(screen.getByRole("button", { name: "Manual" }).getAttribute("aria-current")).toBe("page");
  fireEvent.click(screen.getByRole("button", { name: "Pinned" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/select/5", { method: "POST" });
  fireEvent.click(screen.getByRole("button", { name: "Close Manual" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/close/7", { method: "POST" });
});

test("shows a split as one row of two tabs, each selected by a click", () => {
  render(<App />);
  receive({}, 22);
  const row = document.querySelector<HTMLElement>('.split-row[data-id="20"]');
  const halves = [...(row as HTMLElement).querySelectorAll<HTMLElement>(".half")];
  expect(halves.map((half) => half.textContent)).toEqual(["Left", "Right"]);
  expect(halves[1].getAttribute("aria-current")).toBe("page");
  expect(document.querySelector(".url")?.getAttribute("title")).toBe("https://example.com/right");
  fireEvent.click(halves[0]);
  expect(fetch).toHaveBeenLastCalledWith("/command/select/21", { method: "POST" });
});

test("Option over the sidebar shows a split's separate buttons, until the pointer leaves", () => {
  render(<App />);
  receive({}, 22);
  const sidebar = document.querySelector(".sidebar") as HTMLElement;
  fireEvent.pointerMove(sidebar, { altKey: true });
  expect(sidebar.hasAttribute("data-option")).toBe(true);
  fireEvent.pointerLeave(sidebar);
  expect(sidebar.hasAttribute("data-option")).toBe(false);
  fireEvent.keyDown(sidebar, { key: "Alt", altKey: true });
  expect(sidebar.hasAttribute("data-option")).toBe(true);
  fireEvent.keyUp(sidebar, { key: "Alt" });
  expect(sidebar.hasAttribute("data-option")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Separate Right" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/separate/22", { method: "POST" });
});

test("shows the favorites as a grid of tiles named by their titles", () => {
  render(<App />);
  receive({}, 10);
  const grid = shown().getByRole("list", { name: "Favorites" });
  expect(within(grid).getAllByRole("button")).toHaveLength(2);
  const mail = screen.getByRole("button", { name: "Mail" });
  expect(mail.getAttribute("title")).toBe("Mail");
  expect(mail.textContent).toBe("");
  expect(mail.getAttribute("aria-current")).toBe("page");
  expect(document.querySelector(".url")?.getAttribute("title")).toBe("https://example.com/mail");
  const calendar = screen.getByRole("button", { name: "https://example.com/calendar" });
  expect(calendar.getAttribute("aria-current")).toBeNull();
  expect(screen.queryByRole("button", { name: "Close Mail" })).toBeNull();
});

test("selects a favorite", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Mail" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/select/10", { method: "POST" });
});

test("shows a hint box to drop tabs on when there are no favorites, which its × hides", () => {
  render(<App />);
  receive({}, 3, []);
  const grid = shown().getByRole("list", { name: "Favorites" });
  expect(grid.dataset.parent).toBe("1");
  const hint = grid.querySelector(".favorites-empty") as HTMLElement;
  expect(hint.querySelector("svg[aria-hidden=true]")).not.toBeNull();
  expect(within(hint).getByText("Drag tabs here to keep them close")).toBeDefined();
  expect(within(hint).getByText(/every Space of this profile/)).toBeDefined();
  fireEvent.click(within(grid).getByRole("button", { name: "Hide this hint" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/favorites/hide-hint/13", { method: "POST" });
  receive({}, 3, [], 13, false, true);
  expect(grid.children).toHaveLength(0);
  receive({}, 3, favorites, 13, false, true);
  expect(grid.querySelector(".favorites-empty")).toBeNull();
});

test("balances the favorites' rows, up to 4 tiles a row", () => {
  const columns = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(favoriteColumns);
  expect(columns).toEqual([1, 1, 2, 3, 4, 3, 3, 4, 4, 3, 4]);
});

test("shares the favorites' rows in up to 4 columns", () => {
  render(<App />);
  const columns = () =>
    shown().getByRole("list", { name: "Favorites" }).style.getPropertyValue("--columns");
  receive({}, 3, favorites.slice(0, 1));
  expect(columns()).toBe("1");
  receive({}, 3, favorites);
  expect(columns()).toBe("2");
  const many = Array.from({ length: 6 }, (_, index) => ({ ...favorites[1], id: 20 + index }));
  receive({}, 3, many);
  expect(columns()).toBe("3");
});

test("collapses and expands a folder with a click", () => {
  vi.useFakeTimers();
  render(<App />);
  receive({});
  const docs = screen.getByRole("button", { name: "Docs" });
  expect(docs.getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(docs, { detail: 1 });
  vi.advanceTimersByTime(DOUBLE_CLICK_MS);
  expect(fetch).toHaveBeenLastCalledWith("/command/toggle/6", { method: "POST" });
  fireEvent.click(screen.getByRole("button", { name: "Archive" }), { detail: 0 });
  expect(fetch).toHaveBeenLastCalledWith("/command/toggle/8", { method: "POST" });
  vi.useRealTimers();
});

test("renames a folder with a double click without collapsing it", () => {
  vi.useFakeTimers();
  render(<App />);
  receive({});
  fetch.mockClear();
  const docs = screen.getByRole("button", { name: "Docs" });
  fireEvent.click(docs, { detail: 1 });
  fireEvent.click(docs, { detail: 2 });
  fireEvent.doubleClick(docs);
  vi.advanceTimersByTime(DOUBLE_CLICK_MS);
  expect(fetch).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox", { name: "Folder name" })).toBeDefined();
  vi.useRealTimers();
});

test("the divider's Clear closes today's tabs, and shows only while there are some", () => {
  render(<App />);
  receive({});
  fireEvent.click(shown().getByRole("button", { name: "↓ Clear" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/today/clear", { method: "POST" });
  receive({}, 3, favorites, 14);
  expect(shown().queryByRole("button", { name: "↓ Clear" })).toBeNull();
});

function rename(name: string) {
  fireEvent.doubleClick(screen.getByRole("button", { name }));
  return screen.getByRole("textbox", { name: "Folder name" }) as HTMLInputElement;
}

test("renames a folder after a double click", () => {
  render(<App />);
  receive({});
  const input = rename("Docs");
  expect(input.value).toBe("Docs");
  expect(document.activeElement).toBe(input);
  fireEvent.change(input, { target: { value: " Café/2 " } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(fetch).toHaveBeenLastCalledWith("/command/rename/6/Caf%C3%A9%2F2", { method: "POST" });
  expect(shown().queryByRole("textbox")).toBeNull();
});

test("keeps the title on Escape or when it is blank", () => {
  render(<App />);
  receive({});
  fetch.mockClear();
  let input = rename("Docs");
  fireEvent.change(input, { target: { value: "Other" } });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(shown().queryByRole("textbox")).toBeNull();
  input = rename("Docs");
  fireEvent.change(input, { target: { value: "  " } });
  fireEvent.blur(input);
  expect(shown().queryByRole("textbox")).toBeNull();
  expect(fetch).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Docs" })).toBeDefined();
});

test("saves the title when the field loses the focus", () => {
  render(<App />);
  receive({});
  const input = rename("Archive");
  fireEvent.change(input, { target: { value: "Old things" } });
  fireEvent.blur(input);
  expect(fetch).toHaveBeenLastCalledWith("/command/rename/8/Old%20things", { method: "POST" });
});

test("renames the folder the app has just created", () => {
  render(<App />);
  receive({});
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:rename", { detail: 8 }));
  });
  const input = screen.getByRole("textbox", { name: "Folder name" }) as HTMLInputElement;
  expect(input.value).toBe("Archive");
  expect(document.activeElement).toBe(input);
});

test("Rename Tab edits a tab's name in place; a blank name gives back its page's title", () => {
  render(<App />);
  receive({});
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:rename", { detail: 3 }));
  });
  let input = screen.getByRole("textbox", { name: "Tab name" }) as HTMLInputElement;
  expect(input.value).toBe("Example");
  expect(document.activeElement).toBe(input);
  fireEvent.change(input, { target: { value: " Mine " } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(fetch).toHaveBeenLastCalledWith("/command/rename/3/Mine", { method: "POST" });
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:rename", { detail: 3 }));
  });
  input = screen.getByRole("textbox", { name: "Tab name" }) as HTMLInputElement;
  fireEvent.change(input, { target: { value: "  " } });
  fireEvent.blur(input);
  expect(fetch).toHaveBeenLastCalledWith("/command/rename/3/", { method: "POST" });
  expect(shown().queryByRole("textbox")).toBeNull();
});

test("a muted tab says so on its row", () => {
  render(<App />);
  receive({});
  const muted = (name: string) => shown().getByRole("button", { name }).querySelector(".muted");
  expect(muted("Other")?.getAttribute("title")).toBe("Muted");
  expect(muted("Example")).toBeNull();
});

test("opens the profiles from the footer's menu, and closes them with Escape", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "New" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Profiles…" }));
  expect(screen.queryByRole("menu")).toBeNull();
  expect(screen.getByRole("textbox", { name: "Profile name" })).toHaveProperty("value", "Default");
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(screen.queryByRole("list", { name: "Profiles" })).toBeNull();
});

test("shows one icon per Space at the bottom, the displayed one marked", () => {
  render(<App />);
  receive({});
  const icons = within(screen.getByRole("list", { name: "Spaces" })).getAllByRole("button");
  expect(icons.map((icon) => icon.getAttribute("aria-label"))).toEqual([
    "Home",
    "work",
    "Travel",
    "Books",
  ]);
  expect(icons[0].getAttribute("aria-current")).toBe("true");
  expect(icons[1].hasAttribute("aria-current")).toBe(false);
});

test("switches to a Space when its icon is clicked", () => {
  render(<App />);
  receive({});
  fireEvent.click(switcher().getByRole("button", { name: "work" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/space/14", { method: "POST" });
});

test("tints the sidebar with the displayed Space's color", () => {
  render(<App />);
  receive({});
  const sidebar = screen.getByRole("navigation");
  expect(sidebar.style.getPropertyValue("--space")).toBe("#3e7bfa");
});

test("lays out the displayed Space's page between its neighbors, out of reach", () => {
  render(<App />);
  receive({}, 3, favorites, 14);
  const pages = [...document.querySelectorAll<HTMLElement>(".space-page")];
  expect(pages.map((page) => page.dataset.space)).toEqual(["13", "14", "15"]);
  expect(pages.map((page) => page.hasAttribute("inert"))).toEqual([true, false, true]);
  expect(pages.map((page) => page.style.left)).toEqual(["0%", "100%", "200%"]);
});

// Pages 200 pixels wide, whatever jsdom lays out.
function swipe(dx: number, events: number, interval: number) {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(200);
  const send = (phase: string, dx: number, time: number) =>
    dispatchEvent(new CustomEvent("arbalete:swipe", { detail: { phase, dx, time } }));
  act(() => {
    send("begin", dx, 0);
    for (let i = 1; i < events; i++) send("move", dx, i * interval);
    send("end", 0, events * interval);
  });
}

const frame = () => act(() => new Promise(requestAnimationFrame));
const position = () => (document.querySelector(".pages") as HTMLElement).style.transform;

function reduceMotion() {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: query.includes("reduce") }));
}

// The commands displaying a Space, which pages that are no Space's never send.
const spaceCommands = () =>
  fetch.mock.calls.filter(([url]) => /^\/command\/space\/\d+$/.test(String(url)));
const commands = () => fetch.mock.calls.map(([url]) => String(url));

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("a slow swipe past half way displays the next Space", () => {
  render(<App />);
  receive({});
  swipe(-10, 20, 50);
  expect(fetch).toHaveBeenLastCalledWith("/command/space/14", { method: "POST" });
});

test("a short slow swipe goes back to the displayed Space", () => {
  render(<App />);
  receive({});
  fetch.mockClear();
  swipe(-10, 5, 50);
  expect(fetch).not.toHaveBeenCalled();
});

test("a flick displays the next Space in its direction", () => {
  render(<App />);
  receive({}, 3, favorites, 14);
  swipe(10, 4, 10);
  expect(fetch).toHaveBeenLastCalledWith("/command/space/13", { method: "POST" });
});

test("the library resists a swipe towards a page before it", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  swipe(10, 20, 50);
  await frame();
  expect(position()).toBe("translateX(100%)");
  fetch.mockClear();
  act(() => {
    dispatchEvent(
      new CustomEvent("arbalete:swipe", { detail: { phase: "begin", dx: 1000, time: 0 } }),
    );
  });
  await frame();
  const offset = Number.parseFloat(position().replace("translateX(", ""));
  expect(offset).toBeGreaterThan(100);
  expect(offset).toBeLessThan(125);
  act(() => {
    dispatchEvent(
      new CustomEvent("arbalete:swipe", { detail: { phase: "end", dx: 0, time: 500 } }),
    );
  });
  await frame();
  expect(position()).toBe("translateX(100%)");
  expect(spaceCommands()).toEqual([]);
});

test("without motion, a swipe lands on its page at once", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  swipe(-10, 20, 50);
  await frame();
  expect(position()).toBe("translateX(-100%)");
});

test("a Space displayed otherwise slides its page in", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  expect(position()).toBe("translateX(0%)");
  receive({}, 3, favorites, 16);
  await frame();
  expect(position()).toBe("translateX(-300%)");
  const pages = [...document.querySelectorAll<HTMLElement>(".space-page")];
  expect(pages.map((page) => page.dataset.space)).toEqual(["15", "16"]);
});

test("the first state shows the displayed Space's page at once", () => {
  render(<App />);
  receive({}, 3, favorites, 16);
  expect(position()).toBe("translateX(-300%)");
});

const newSpace = () => document.querySelector<HTMLElement>(".new-space-page");
const nameOfNew = () => screen.getByRole("textbox", { name: "New Space name" });

test("a swipe past the last Space shows the page creating one, without telling the app", async () => {
  reduceMotion();
  render(<App />);
  receive({}, 3, favorites, 16);
  fetch.mockClear();
  expect(newSpace()?.style.left).toBe("400%");
  expect(newSpace()?.hasAttribute("inert")).toBe(true);
  swipe(-10, 20, 50);
  await frame();
  expect(position()).toBe("translateX(-400%)");
  expect(newSpace()?.hasAttribute("inert")).toBe(false);
  expect(document.querySelectorAll(".space-page:not([inert])")).toHaveLength(0);
  expect(document.activeElement).toBe(nameOfNew());
  expect(screen.getByRole("navigation").style.getPropertyValue("--tinted")).toBe("0");
  expect(fetch).not.toHaveBeenCalled();
});

// Touches and moves without lifting, the pointer `y` points from the top.
function pull(dx: number, events: number, y = 0) {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(200);
  const send = (phase: string, time: number) =>
    dispatchEvent(new CustomEvent("arbalete:swipe", { detail: { phase, dx, y, time } }));
  act(() => {
    send("begin", 0);
    for (let i = 1; i < events; i++) send("move", i * 50);
  });
}

function lift() {
  act(() => {
    dispatchEvent(
      new CustomEvent("arbalete:swipe", { detail: { phase: "end", dx: 0, y: 0, time: 5000 } }),
    );
  });
}

const bubble = () => document.querySelector<HTMLElement>(".new-bubble");

test("a pull past the last Space brings out a + at the pointer, which arms further on", async () => {
  render(<App />);
  receive({}, 3, favorites, 16);
  pull(-10, 2, 120);
  await frame();
  expect(Number(bubble()?.style.opacity)).toBeGreaterThan(0);
  expect(Number(bubble()?.style.opacity)).toBeLessThan(1);
  expect(bubble()?.style.transform).toContain("120px");
  expect(bubble()?.hasAttribute("data-armed")).toBe(false);
  // The pages resist instead of showing the page creating a Space.
  const offset = Number.parseFloat(position().replace("translateX(", ""));
  expect(offset).toBeLessThan(-300);
  expect(offset).toBeGreaterThan(-320);
  pull(-10, 8, 120);
  await frame();
  expect(bubble()?.style.opacity).toBe("1");
  expect(bubble()?.hasAttribute("data-armed")).toBe(true);
});

test("releasing the + before it arms goes back to the last Space", async () => {
  reduceMotion();
  render(<App />);
  receive({}, 3, favorites, 16);
  pull(-10, 2);
  lift();
  await frame();
  expect(position()).toBe("translateX(-300%)");
  expect(newSpace()?.hasAttribute("inert")).toBe(true);
  expect(bubble()?.style.opacity).toBe("0");
});

test("releasing the + armed opens the page creating a Space", async () => {
  reduceMotion();
  render(<App />);
  receive({}, 3, favorites, 16);
  pull(-10, 8);
  lift();
  await frame();
  expect(position()).toBe("translateX(-400%)");
  expect(newSpace()?.hasAttribute("inert")).toBe(false);
});

test("on the way to the library, the card moves right and the first Space fades", async () => {
  render(<App />);
  receive({});
  pull(10, 6);
  await frame();
  const offsets = commands()
    .filter((url) => url.startsWith("/command/card/offset/"))
    .map((url) => Number(url.split("/").at(-1)));
  expect(offsets.at(-1)).toBeGreaterThan(0);
  expect(offsets.at(-1)).toBeLessThan(180);
  const first = document.querySelector<HTMLElement>('.space-page[data-space="13"]');
  expect(Number(first?.style.opacity)).toBeLessThan(1);
  expect(commands()).not.toContain("/command/library/show");
});

test("swiping back from the new Space's page leaves the Spaces as they were", async () => {
  reduceMotion();
  render(<App />);
  receive({}, 3, favorites, 16);
  swipe(-10, 20, 50);
  fireEvent.change(nameOfNew(), { target: { value: "Reading" } });
  fetch.mockClear();
  swipe(10, 20, 50);
  await frame();
  expect(position()).toBe("translateX(-300%)");
  expect(newSpace()?.hasAttribute("inert")).toBe(true);
  expect(shown()).toBeDefined();
  expect(fetch).not.toHaveBeenCalled();
  expect((nameOfNew() as HTMLInputElement).value).toBe("");
});

test("the footer menu's New Space slides to that page, and its Space icon back", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "New" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "New Space" }));
  await frame();
  expect(position()).toBe("translateX(-400%)");
  expect(switcher().getByRole("button", { name: "Home" }).hasAttribute("aria-current")).toBe(false);
  fireEvent.click(switcher().getByRole("button", { name: "Home" }));
  await frame();
  expect(position()).toBe("translateX(0%)");
  expect(switcher().getByRole("button", { name: "Home" }).getAttribute("aria-current")).toBe(
    "true",
  );
  expect(fetch).not.toHaveBeenCalledWith("/command/space/13", { method: "POST" });
});

test("creating a Space shows its page where the new Space's was", async () => {
  reduceMotion();
  render(<App />);
  receive({}, 3, favorites, 16);
  swipe(-10, 20, 50);
  fireEvent.change(nameOfNew(), { target: { value: "Reading" } });
  fireEvent.submit(nameOfNew());
  expect(fetch).toHaveBeenLastCalledWith("/command/space/create/purple//Reading", {
    method: "POST",
  });
  spaces.push({ id: 17, title: "Reading", emoji: null, color: "purple", profile: 1, pinned: [] });
  try {
    receive({}, 3, favorites, 17);
    await frame();
    expect(position()).toBe("translateX(-400%)");
    expect(document.querySelector<HTMLElement>(".space-page:not([inert])")?.dataset.space).toBe(
      "17",
    );
    expect(newSpace()?.style.left).toBe("500%");
    expect(newSpace()?.hasAttribute("inert")).toBe(true);
  } finally {
    spaces.pop();
  }
});

test("a tab's menu moving it into a new Space slides to the page creating one", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:new-space", { detail: 3 }));
  });
  await frame();
  expect(position()).toBe("translateX(-400%)");
  fireEvent.change(nameOfNew(), { target: { value: "Reading" } });
  fireEvent.submit(nameOfNew());
  expect(fetch).toHaveBeenLastCalledWith("/command/space/create/purple//Reading/3", {
    method: "POST",
  });
});

const library = () => document.querySelector<HTMLElement>(".library");

test("a swipe before the first Space shows the library, without telling the app", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  fetch.mockClear();
  expect(library()?.style.left).toBe("-100%");
  expect(library()?.hasAttribute("inert")).toBe(true);
  swipe(10, 20, 50);
  await frame();
  expect(position()).toBe("translateX(100%)");
  expect(library()?.hasAttribute("inert")).toBe(false);
  expect(document.querySelectorAll(".space-page:not([inert])")).toHaveLength(0);
  expect(screen.getByRole("navigation").style.getPropertyValue("--tinted")).toBe("0");
  expect(spaceCommands()).toEqual([]);
  // The app widens the sidebar for the library, the card resting beside it.
  expect(commands().at(-1)).toBe("/command/library/show");
});

test("the Library button slides to the library, like a swipe", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  fetch.mockClear();
  const sidebar = screen.getByRole("navigation");
  expect(sidebar.hasAttribute("data-library")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Library" }));
  await frame();
  expect(position()).toBe("translateX(100%)");
  expect(library()?.hasAttribute("inert")).toBe(false);
  // The library takes the whole sidebar, without the url bar and the Spaces' footer.
  expect(sidebar.hasAttribute("data-library")).toBe(true);
  expect(spaceCommands()).toEqual([]);
});

test("Window › Library slides to the library, on the section it showed last", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  fireEvent.click(within(library() as HTMLElement).getByRole("tab", { name: "History" }));
  act(() => {
    dispatchEvent(new Event("arbalete:open-library"));
  });
  await frame();
  expect(position()).toBe("translateX(100%)");
  expect(
    within(library() as HTMLElement)
      .getByRole("tab", { name: "History" })
      .getAttribute("aria-selected"),
  ).toBe("true");
});

test("the Space menu's Manage Spaces slides to the library", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:library", { detail: null }));
  });
  await frame();
  expect(position()).toBe("translateX(100%)");
  expect(library()?.hasAttribute("inert")).toBe(false);
});

test("the library reopens a closed tab in the displayed Space and slides back to it", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  swipe(10, 20, 50);
  await frame();
  const shownLibrary = within(library() as HTMLElement);
  fireEvent.click(shownLibrary.getByRole("tab", { name: "Recently Closed" }));
  fireEvent.click(shownLibrary.getByRole("button", { name: /^Gone/ }));
  expect(fetch).toHaveBeenCalledWith("/command/reopen/0", { method: "POST" });
  await frame();
  expect(position()).toBe("translateX(0%)");
  expect(library()?.hasAttribute("inert")).toBe(true);
  expect(screen.getByRole("navigation").hasAttribute("data-library")).toBe(false);
  expect(commands().at(-1)).toBe("/command/card/offset/0");
});

test("the library's Space cards show a Space, create one, or go back to the displayed one", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  const shownLibrary = () => within(library() as HTMLElement);
  const open = async () => {
    fireEvent.click(screen.getByRole("button", { name: "Library" }));
    await frame();
    expect(position()).toBe("translateX(100%)");
  };
  await open();
  fireEvent.click(shownLibrary().getByRole("button", { name: "Show Travel" }), { detail: 0 });
  expect(spaceCommands().at(-1)?.[0]).toBe("/command/space/15");
  await open();
  fireEvent.click(shownLibrary().getByRole("button", { name: "New Space" }));
  await frame();
  expect(position()).toBe("translateX(-400%)");
  await open();
  fireEvent.click(shownLibrary().getByRole("button", { name: "Back to the Space" }));
  await frame();
  expect(position()).toBe("translateX(0%)");
});

test("Spaces reordered from the library leave it in view, the displayed Space unchanged", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Library" }));
  await frame();
  const home = spaces.shift() as Space;
  spaces.push(home);
  try {
    receive({});
    await frame();
    expect(position()).toBe("translateX(100%)");
    expect(library()?.hasAttribute("inert")).toBe(false);
    expect(
      [...document.querySelectorAll(".space-card-name")].map((name) => name.textContent),
    ).toEqual(["work", "Travel", "Books", "Home"]);
    // Back on the displayed Space, its page is now the last one.
    fireEvent.click(
      within(library() as HTMLElement).getByRole("button", { name: "Back to the Space" }),
    );
    await frame();
    expect(position()).toBe("translateX(-300%)");
  } finally {
    spaces.unshift(spaces.pop() as Space);
  }
});

test("asks the app to paint the window with the displayed Space's color, blending on the way", async () => {
  const root = document.documentElement.style;
  root.setProperty("--window-end", "#1b0e1f");
  root.setProperty("--panel", "#1e1a28");
  const scheme = { addEventListener: vi.fn(), removeEventListener: vi.fn() };
  vi.stubGlobal("matchMedia", () => scheme);
  try {
    render(<App />);
    receive({});
    const blue = { color: COLORS.blue, strength: 1 };
    const green = { color: COLORS.green, strength: 1 };
    expect(commands()).toContain(
      `/command/${windowColors(blue, COLORS.blue, "#1b0e1f", "#1e1a28")}`,
    );
    receive({}, 3, favorites, 14);
    // The window's color blends on the way, an empty Space's card keeps the displayed Space's.
    expect(commands()).toContain(
      `/command/${windowColors(blue, COLORS.green, "#1b0e1f", "#1e1a28")}`,
    );
    await act(async () => {
      for (let i = 0; i < 60; i++) await new Promise(requestAnimationFrame);
    });
    const blends = commands().filter((url) => url.includes("/window/colors/"));
    expect(blends.length).toBeGreaterThan(3);
    expect(fetch).toHaveBeenLastCalledWith(
      `/command/${windowColors(green, COLORS.green, "#1b0e1f", "#1e1a28")}`,
      { method: "POST" },
    );
    // A change of appearance changes the colors they derive from.
    root.setProperty("--window-end", "#e7e2f1");
    act(() => scheme.addEventListener.mock.lastCall?.[1]());
    expect(fetch).toHaveBeenLastCalledWith(
      `/command/${windowColors(green, COLORS.green, "#e7e2f1", "#1e1a28")}`,
      { method: "POST" },
    );
  } finally {
    root.removeProperty("--window-end");
    root.removeProperty("--panel");
    vi.unstubAllGlobals();
    vi.stubGlobal("fetch", fetch);
  }
});

// Whether the right-click leaves its default menu to show.
const rightClick = (element: Element) => fireEvent.contextMenu(element);

test("a right-click on a row asks the app for its menu, and shows no page menu", () => {
  render(<App />);
  receive({});
  const page = shown();
  for (const [row, id] of [
    [page.getByRole("button", { name: "Other" }), 4],
    [page.getByRole("button", { name: "Manual" }), 7],
    [page.getByRole("button", { name: "Docs" }), 6],
    [page.getByRole("button", { name: "Mail" }), 10],
    [page.getByRole("button", { name: "Close Example" }), 3],
    [page.getByRole("button", { name: "Right" }), 22],
  ] as const) {
    fetch.mockClear();
    expect(rightClick(row)).toBe(false);
    expect(fetch).toHaveBeenCalledExactlyOnceWith(`/command/menu/node/${id}`, { method: "POST" });
  }
});

test("a right-click elsewhere shows no menu, but in a text field", () => {
  render(<App />);
  receive({});
  fetch.mockClear();
  expect(rightClick(screen.getByRole("button", { name: "Reload" }))).toBe(false);
  expect(rightClick(document.querySelector(".divider") as Element)).toBe(false);
  expect(fetch).not.toHaveBeenCalled();
  fireEvent.doubleClick(shown().getByRole("button", { name: "Docs" }));
  expect(rightClick(screen.getByRole("textbox", { name: "Folder name" }))).toBe(true);
  expect(fetch).not.toHaveBeenCalledWith(expect.stringContaining("/menu/"), expect.anything());
});

// Drags `row` with the pointer to `y` px from the top of every row, which the layout below puts
// at 0, `under` being under the pointer; the favorites lie far above.
function dragTo(row: HTMLElement, under: Element, y: number) {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return this.matches(".favorites") ? new DOMRect(0, -500, 250, 47) : new DOMRect(0, 0, 250, 40);
  });
  document.elementFromPoint = () => under;
  fireEvent.pointerDown(row, { button: 0, clientX: 20, clientY: 10 });
  fireEvent.pointerMove(window, { clientX: 20, clientY: y });
}

const dragCommands = () =>
  fetch.mock.calls
    .map(([url]) => String(url))
    .filter((url) => /\/(move|split|duplicate|split-duplicate)\//.test(url));

test("dragging a row leaves it dimmed in place, a ghost of it following the pointer", () => {
  render(<App />);
  receive({});
  const other = shown().getByRole("button", { name: "Other" });
  dragTo(other, shown().getByRole("button", { name: "Example" }), 4);
  expect(other.closest("li")?.classList.contains("dragged")).toBe(true);
  expect(document.querySelector(".drag-ghost .ghost-pill")?.textContent).toBe("Other");
  expect(document.querySelector(".drag-ghost.copy")).toBeNull();
  const line = document.querySelector<HTMLElement>(".drop-indicator");
  expect(line?.style.display).toBe("block");
  fireEvent.pointerUp(window);
  expect(dragCommands()).toEqual(["/command/move/4/12/0"]);
  expect(document.querySelector(".drag-ghost")).toBeNull();
  expect(line?.style.display).toBe("");
  vi.restoreAllMocks();
});

test("holding Option while dragging a tab drops a copy of it, as a badge on the ghost says", () => {
  render(<App />);
  receive({});
  const other = shown().getByRole("button", { name: "Other" });
  dragTo(other, shown().getByRole("button", { name: "Example" }), 4);
  fireEvent.pointerMove(window, { clientX: 20, clientY: 4, altKey: true });
  expect(document.querySelector(".drag-ghost.copy")).not.toBeNull();
  expect(other.closest("li")?.classList.contains("dragged")).toBe(false);
  fireEvent.keyUp(window, { key: "Alt" });
  expect(document.querySelector(".drag-ghost.copy")).toBeNull();
  fireEvent.keyDown(window, { key: "Alt", altKey: true });
  expect(document.querySelector(".drag-ghost.copy")).not.toBeNull();
  fireEvent.pointerUp(window, { altKey: true });
  expect(dragCommands()).toEqual(["/command/duplicate/4/12/0"]);
  vi.restoreAllMocks();
});

test("Option dropped on the middle of a tab row splits a copy of the tab with it", () => {
  render(<App />);
  receive({});
  const other = shown().getByRole("button", { name: "Other" });
  dragTo(other, shown().getByRole("button", { name: "Example" }), 20);
  fireEvent.pointerUp(window, { altKey: true });
  expect(dragCommands()).toEqual(["/command/split-duplicate/4/3"]);
  vi.restoreAllMocks();
});

test("over the middle of a tab row, a dragged tab previews the split it drops as", () => {
  render(<App />);
  receive({});
  const other = shown().getByRole("button", { name: "Other" });
  dragTo(other, shown().getByRole("button", { name: "Example" }), 20);
  const halves = [...document.querySelectorAll(".split-preview .half")];
  expect(halves.map((half) => half.textContent)).toEqual(["Example", "Other"]);
  expect(document.querySelector(".split-target")?.getAttribute("data-id")).toBe("3");
  expect(document.querySelector<HTMLElement>(".drop-indicator")?.style.display).toBe("");
  fireEvent.pointerUp(window);
  expect(dragCommands()).toEqual(["/command/split/4/3"]);
  expect(document.querySelector(".split-preview, .split-target")).toBeNull();
  vi.restoreAllMocks();
});

test("over full favorites, a dragged tab opens no slot, says they are full, and drops nothing", () => {
  vi.useFakeTimers();
  render(<App />);
  const many = Array.from({ length: MAX_FAVORITES }, (_, i) => ({
    id: 100 + i,
    url: `https://example.com/${i}`,
    title: `F${i}`,
    favicon: null,
  }));
  receive({}, 3, many);
  const other = shown().getByRole("button", { name: "Other" });
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return this.matches(".favorites") ? new DOMRect(0, -500, 250, 160) : new DOMRect(0, 0, 250, 40);
  });
  fireEvent.pointerDown(other, { button: 0, clientX: 20, clientY: 10 });
  fireEvent.pointerMove(window, { clientX: 20, clientY: -450 });
  expect(shown().getByRole("status").textContent).toBe("Favorites are full");
  expect(document.querySelector(".favorite-slot")).toBeNull();
  fireEvent.pointerUp(window);
  expect(dragCommands()).toEqual([]);
  expect(shown().getByRole("status")).toBeDefined();
  act(() => vi.advanceTimersByTime(FULL_HINT_MS));
  expect(shown().queryByRole("status")).toBeNull();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

test("Escape cancels a drag: nothing moves, and the release selects nothing", () => {
  render(<App />);
  receive({});
  const other = shown().getByRole("button", { name: "Other" });
  dragTo(other, shown().getByRole("button", { name: "Example" }), 4);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(other.closest("li")?.classList.contains("dragged")).toBe(false);
  expect(document.querySelector(".drag-ghost")).toBeNull();
  fireEvent.pointerUp(window);
  fireEvent.click(other);
  expect(dragCommands()).toEqual([]);
  expect(fetch).not.toHaveBeenCalledWith("/command/select/4", expect.anything());
  vi.restoreAllMocks();
});
