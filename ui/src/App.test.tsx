import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { App, DOUBLE_CLICK_MS } from "./App";
import type { Item, Page, Tab } from "./bridge";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const today: Tab[] = [
  { id: 3, url: "https://example.com/", title: "Example", favicon: null },
  { id: 4, url: "https://example.org/", title: "Other", favicon: null },
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

const spaces = [
  { id: 13, title: "Home", emoji: null, color: "blue" },
  { id: 14, title: "work", emoji: null, color: "green" },
  { id: 15, title: "Travel", emoji: null, color: "red" },
  { id: 16, title: "Books", emoji: null, color: "gray" },
];

// The first Space shows the tabs above, the others none. The app sends the pages of the displayed
// Space and its neighbors.
function receive(page: Partial<Page>, active = 3, favoriteTabs = favorites, displayed = 13) {
  const empty = { favorites: [], pinned: [], today: [], selected: null };
  const shown = spaces.findIndex(({ id }) => id === displayed);
  const detail = {
    page: { progress: 1, loading: false, canGoBack: true, canGoForward: true, ...page },
    space: displayed,
    spaces,
    pages: spaces
      .map(({ id }, index) => ({
        space: id,
        roots: { favorites: 1, pinned: 2 + index * 100, today: 12 + index * 100 },
        ...(index === 0
          ? { favorites: favoriteTabs, pinned, today, selected: active }
          : { ...empty }),
      }))
      .filter((_, index) => Math.abs(index - shown) <= 1),
    profiles: [{ id: 1, name: "Default", color: "purple" }],
  };
  act(() => {
    dispatchEvent(new CustomEvent("arbalette:state", { detail }));
  });
}

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

test("sends navigation commands to the app", () => {
  render(<App />);
  receive({});
  for (const [label, command] of [
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
  expect(screen.getByText("https://example.com/")).toBeDefined();
  expect(screen.getByRole("progressbar", { name: "Loading" })).toBeDefined();
  expect(screen.getByRole("button", { name: "Back" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "Example" }).getAttribute("aria-current")).toBe("page");
});

test("keeps the url bar without a selected tab", () => {
  render(<App />);
  receive({ loading: true }, 99);
  const url = document.querySelector(".url");
  expect(url?.textContent).toBe("");
  expect(screen.queryByRole("progressbar")).toBeNull();
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
  expect(screen.getByText("https://example.net/docs")).toBeDefined();
  expect(screen.getByRole("button", { name: "Manual" }).getAttribute("aria-current")).toBe("page");
  fireEvent.click(screen.getByRole("button", { name: "Pinned" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/select/5", { method: "POST" });
  fireEvent.click(screen.getByRole("button", { name: "Close Manual" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/close/7", { method: "POST" });
});

test("shows the favorites as a grid of tiles named by their titles", () => {
  render(<App />);
  receive({}, 10);
  const grid = screen.getByRole("list", { name: "Favorites" });
  expect(within(grid).getAllByRole("button")).toHaveLength(2);
  const mail = screen.getByRole("button", { name: "Mail" });
  expect(mail.getAttribute("title")).toBe("Mail");
  expect(mail.textContent).toBe("");
  expect(mail.getAttribute("aria-current")).toBe("page");
  expect(screen.getByText("https://example.com/mail")).toBeDefined();
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

test("hides the favorites when there are none", () => {
  render(<App />);
  receive({}, 3, []);
  expect(screen.queryByRole("list", { name: "Favorites" })).toBeNull();
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

test("creates a folder from the divider", () => {
  render(<App />);
  receive({});
  fireEvent.click(shown().getByRole("button", { name: "New folder" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/folder/new", { method: "POST" });
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
  expect(screen.queryByRole("textbox")).toBeNull();
});

test("keeps the title on Escape or when it is blank", () => {
  render(<App />);
  receive({});
  fetch.mockClear();
  let input = rename("Docs");
  fireEvent.change(input, { target: { value: "Other" } });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(screen.queryByRole("textbox")).toBeNull();
  input = rename("Docs");
  fireEvent.change(input, { target: { value: "  " } });
  fireEvent.blur(input);
  expect(screen.queryByRole("textbox")).toBeNull();
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
    dispatchEvent(new CustomEvent("arbalette:rename", { detail: 8 }));
  });
  const input = screen.getByRole("textbox", { name: "Folder name" }) as HTMLInputElement;
  expect(input.value).toBe("Archive");
  expect(document.activeElement).toBe(input);
});

test("shows the profiles control at the bottom of the sidebar", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "Profiles" }));
  expect(screen.getByRole("textbox", { name: "Profile name" })).toHaveProperty("value", "Default");
});

test("shows one icon per Space at the bottom, the displayed one marked", () => {
  render(<App />);
  receive({});
  const icons = within(screen.getByRole("list", { name: "Spaces" })).getAllByRole("button");
  expect(icons.map((icon) => icon.textContent)).toEqual(["H", "W", "T", "B", "+"]);
  expect(icons[0].getAttribute("aria-current")).toBe("true");
  expect(icons[1].hasAttribute("aria-current")).toBe(false);
});

test("switches to a Space when its icon is clicked", () => {
  render(<App />);
  receive({});
  fireEvent.click(screen.getByRole("button", { name: "work" }));
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
    dispatchEvent(new CustomEvent("arbalette:swipe", { detail: { phase, dx, time } }));
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

test("the first Space resists a swipe towards the one before", async () => {
  reduceMotion();
  render(<App />);
  receive({});
  fetch.mockClear();
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(200);
  act(() => {
    dispatchEvent(
      new CustomEvent("arbalette:swipe", { detail: { phase: "begin", dx: 1000, time: 0 } }),
    );
  });
  await frame();
  const offset = Number.parseFloat(position().replace("translateX(", ""));
  expect(offset).toBeGreaterThan(0);
  expect(offset).toBeLessThan(25);
  act(() => {
    dispatchEvent(
      new CustomEvent("arbalette:swipe", { detail: { phase: "end", dx: 0, time: 500 } }),
    );
  });
  await frame();
  expect(position()).toBe("translateX(0%)");
  expect(fetch).not.toHaveBeenCalled();
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
