import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { ClosedTab, Space } from "./bridge";
import { ago, Library, LibraryButton, POPOVER_DELAY_MS } from "./Library";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const spaces: Space[] = [
  { id: 13, title: "Home", emoji: "🏠", color: "blue", profile: 1, pinned: [] },
  {
    id: 14,
    title: "Work",
    emoji: null,
    color: "green",
    profile: 1,
    pinned: [{ id: 3, url: "https://mail.test/", title: "Mail", favicon: null }],
  },
];

function renderLibrary(closed: ClosedTab[] = [], onBack = () => {}) {
  return render(
    <Library
      closed={closed}
      downloads={[]}
      spaces={spaces}
      profiles={[{ id: 1, name: "Default", color: "purple" }]}
      shown
      onBack={onBack}
      onShow={() => {}}
      onNewSpace={() => {}}
    />,
  );
}

const section = (name: string) => screen.getByRole("tab", { name });
const search = () => screen.getByRole("searchbox");

test("a rail of sections opens on the Spaces, and the one picked shows under its search field", () => {
  renderLibrary();
  const tabs = screen.getAllByRole("tab");
  expect(tabs.map((tab) => tab.textContent)).toEqual([
    "Spaces",
    "Recently Closed",
    "History",
    "Downloads",
  ]);
  expect(section("Spaces").getAttribute("aria-selected")).toBe("true");
  // The Spaces, which fit at a glance, have no search field.
  expect(screen.queryByRole("searchbox")).toBeNull();
  expect(
    screen.getAllByRole("listitem").filter((item) => item.matches(".space-card")),
  ).toHaveLength(2);
  // The Spaces fill the window; the other sections rest beside the page.
  expect(fetch).toHaveBeenLastCalledWith("/command/library/wide", { method: "POST" });
  fireEvent.click(section("Recently Closed"));
  expect(fetch).toHaveBeenLastCalledWith("/command/library/narrow", { method: "POST" });
  expect(section("Recently Closed").getAttribute("aria-selected")).toBe("true");
  expect(section("Spaces").getAttribute("aria-selected")).toBe("false");
  expect(search().getAttribute("placeholder")).toBe("Search Recently Closed…");
  expect(screen.getByRole("tabpanel").getAttribute("aria-label")).toBe("Recently Closed");
  expect(document.querySelector(".space-card")).toBeNull();
});

test("the section picked stays picked until the Space menu's Manage Spaces asks for the Spaces", () => {
  const { rerender } = renderLibrary();
  fireEvent.click(section("Recently Closed"));
  rerender(
    <Library
      closed={[]}
      downloads={[]}
      spaces={spaces}
      profiles={[]}
      shown={false}
      onBack={() => {}}
      onShow={() => {}}
      onNewSpace={() => {}}
    />,
  );
  expect(section("Recently Closed").getAttribute("aria-selected")).toBe("true");
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:library", { detail: null }));
  });
  expect(section("Spaces").getAttribute("aria-selected")).toBe("true");
});

test("lists the tabs closed since launch, the last closed first, and reopens one by rank", () => {
  vi.useFakeTimers();
  vi.setSystemTime(3 * 60_000);
  const onBack = vi.fn();
  const closed = [
    { url: "https://example.com/last", title: "Last", favicon: null, closedAt: 60_000 },
    { url: "https://example.com/untitled", title: "", favicon: null, closedAt: 0 },
  ];
  renderLibrary(closed, onBack);
  fireEvent.click(section("Recently Closed"));
  const rows = within(screen.getByRole("tabpanel")).getAllByRole("button");
  expect(rows.map((row) => row.textContent)).toEqual([
    "Last2 min ago",
    "https://example.com/untitled3 min ago",
  ]);
  fireEvent.click(rows[1]);
  expect(fetch).toHaveBeenLastCalledWith("/command/reopen/1", { method: "POST" });
  expect(onBack).toHaveBeenCalledOnce();
});

test("searching the closed tabs keeps those whose title or url holds the text, whatever its case", () => {
  const closed = [
    { url: "https://news.test/", title: "Morning News", favicon: null, closedAt: 0 },
    { url: "https://docs.test/guide", title: "Guide", favicon: null, closedAt: 0 },
  ];
  renderLibrary(closed);
  fireEvent.click(section("Recently Closed"));
  const titles = () =>
    [...document.querySelectorAll(".closed-tab .title")].map((t) => t.textContent);
  fireEvent.change(search(), { target: { value: "news" } });
  expect(titles()).toEqual(["Morning News"]);
  fireEvent.change(search(), { target: { value: "DOCS" } });
  expect(titles()).toEqual(["Guide"]);
  // It still reopens the tab by its rank among all the closed tabs.
  fireEvent.click(screen.getByRole("button", { name: /Guide/ }));
  expect(fetch).toHaveBeenLastCalledWith("/command/reopen/1", { method: "POST" });
  fireEvent.change(search(), { target: { value: "nothing" } });
  expect(screen.getByText("No closed tab matches")).toBeDefined();
  // Another section starts with an empty field.
  fireEvent.click(section("History"));
  expect((search() as HTMLInputElement).value).toBe("");
});

test("the rail's arrow goes back to the Space", () => {
  const onBack = vi.fn();
  renderLibrary([], onBack);
  fireEvent.click(screen.getByRole("button", { name: "Back to the Space" }));
  expect(onBack).toHaveBeenCalledOnce();
});

test("says when no tab has been closed", () => {
  renderLibrary();
  fireEvent.click(section("Recently Closed"));
  expect(screen.getByText("No tab closed yet")).toBeDefined();
});

test("tells how long ago a tab closed in a few words", () => {
  const minute = 60_000;
  expect(ago(0, 30_000)).toBe("Just now");
  expect(ago(0, 2 * minute)).toBe("2 min ago");
  expect(ago(0, 59 * minute)).toBe("59 min ago");
  expect(ago(0, 125 * minute)).toBe("2 h ago");
  expect(ago(0, 49 * 60 * minute)).toBe("2 d ago");
});

test("resting on the Library button shows the closed tabs above it, which reopen; leaving hides them", () => {
  vi.useFakeTimers();
  vi.setSystemTime(10 * 60_000);
  const onOpen = vi.fn();
  const onReopen = vi.fn();
  const closed = [
    { url: "https://example.com/a", title: "A", favicon: null, closedAt: 8 * 60_000 },
    { url: "https://example.com/b", title: "B", favicon: "https://example.com/b.ico", closedAt: 0 },
  ];
  const { container } = render(
    <LibraryButton closed={closed} onOpen={onOpen} onReopen={onReopen} />,
  );
  const wrapper = container.firstElementChild as Element;
  const popover = () => screen.queryByRole("region", { name: "Recently closed" });
  fireEvent.mouseEnter(wrapper);
  act(() => vi.advanceTimersByTime(POPOVER_DELAY_MS - 1));
  expect(popover()).toBeNull();
  act(() => vi.advanceTimersByTime(1));
  const items = within(popover() as HTMLElement).getAllByRole("button");
  expect(items.map((item) => item.textContent)).toEqual(["A2 min ago", "B10 min ago"]);
  expect(items[1].querySelector("img")?.getAttribute("src")).toBe("https://example.com/b.ico");
  expect(items[0].querySelector(".favicon.placeholder")).not.toBeNull();
  fireEvent.click(items[1]);
  expect(fetch).toHaveBeenLastCalledWith("/command/reopen/1", { method: "POST" });
  expect(onReopen).toHaveBeenCalledOnce();
  expect(popover()).toBeNull();
  fireEvent.mouseEnter(wrapper);
  act(() => vi.advanceTimersByTime(POPOVER_DELAY_MS));
  expect(popover()).not.toBeNull();
  fireEvent.mouseLeave(wrapper);
  expect(popover()).toBeNull();
  // Leaving before the delay shows nothing.
  fireEvent.mouseEnter(wrapper);
  fireEvent.mouseLeave(wrapper);
  act(() => vi.advanceTimersByTime(POPOVER_DELAY_MS));
  expect(popover()).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Library" }));
  expect(onOpen).toHaveBeenCalledOnce();
});

test("the popover says when no tab has been closed", () => {
  vi.useFakeTimers();
  const { container } = render(<LibraryButton closed={[]} onOpen={() => {}} onReopen={() => {}} />);
  fireEvent.mouseEnter(container.firstElementChild as Element);
  act(() => vi.advanceTimersByTime(POPOVER_DELAY_MS));
  expect(screen.getByText("No tab closed yet")).toBeDefined();
});
