import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
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

test("lists the tabs closed since launch, the last closed first, and reopens one by rank", () => {
  const onReopen = vi.fn();
  const closed = [
    { url: "https://example.com/last", title: "Last", favicon: null, closedAt: 0 },
    { url: "https://example.com/untitled", title: "", favicon: null, closedAt: 0 },
  ];
  render(<Library closed={closed} shown onReopen={onReopen} />);
  const rows = screen.getAllByRole("button");
  expect(rows.map((row) => row.textContent)).toEqual(["Last", "https://example.com/untitled"]);
  fireEvent.click(rows[1]);
  expect(fetch).toHaveBeenLastCalledWith("/command/reopen/1", { method: "POST" });
  expect(onReopen).toHaveBeenCalledOnce();
});

test("says when no tab has been closed", () => {
  render(<Library closed={[]} shown onReopen={() => {}} />);
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
