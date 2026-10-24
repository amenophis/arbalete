import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { App } from "./App";
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

function receive(page: Partial<Page>, active = 3, favoriteTabs = favorites) {
  const detail = {
    page: { progress: 1, loading: false, canGoBack: true, canGoForward: true, ...page },
    favorites: favoriteTabs,
    pinned,
    today,
    active,
  };
  act(() => {
    dispatchEvent(new CustomEvent("arbalette:state", { detail }));
  });
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
  const divider = screen.getByRole("separator");
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
