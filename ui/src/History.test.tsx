import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { HistoryList, PAGE_SIZE, type Visit } from "./History";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const command = (path: string) => [`/command/${path}`, { method: "POST" }];

const NOW = new Date(2026, 9, 26, 15, 0).getTime();
const YESTERDAY = NOW - 24 * 60 * 60 * 1000;

const visit = (url: string, title: string, lastVisit: number): Visit => ({ url, title, lastVisit });

function answer(text: string, entries: Visit[]) {
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:history", { detail: { text, entries } }));
  });
}

test("searches the history as the query changes, and keeps only the latest answer", () => {
  const { rerender } = render(<HistoryList query="" now={NOW} onOpen={() => {}} />);
  expect(fetch).toHaveBeenLastCalledWith(...command(`history/search/${PAGE_SIZE}/`));
  rerender(<HistoryList query="café" now={NOW} onOpen={() => {}} />);
  expect(fetch).toHaveBeenLastCalledWith(...command(`history/search/${PAGE_SIZE}/caf%C3%A9`));
  answer("", [visit("https://old.example/", "Old answer", NOW)]);
  expect(screen.queryByText("Old answer")).toBeNull();
  answer("café", []);
  expect(screen.getByText("No page matches")).toBeTruthy();
});

test("lists the pages by day with their time and site, and opens one in a tab", () => {
  const onOpen = vi.fn();
  const { container } = render(<HistoryList query="" now={NOW} onOpen={onOpen} />);
  answer("", [
    visit("https://www.example.com/a", "Example", NOW - 3_600_000),
    visit("https://news.test/b", "", YESTERDAY),
  ]);
  const headings = [...container.querySelectorAll("h3")].map((heading) => heading.textContent);
  expect(headings).toEqual(["Today", "Yesterday"]);
  expect(screen.getByText("https://news.test/b")).toBeTruthy();
  const time = new Date(NOW - 3_600_000).toLocaleTimeString(undefined, { timeStyle: "short" });
  expect(screen.getByText(`${time} · example.com`)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /Example/ }));
  expect(fetch).toHaveBeenLastCalledWith(...command("open/https%3A%2F%2Fwww.example.com%2Fa"));
  expect(onOpen).toHaveBeenCalled();
});

test("forgets a page", () => {
  render(<HistoryList query="" now={NOW} onOpen={() => {}} />);
  answer("", [visit("https://a.test/", "A", NOW), visit("https://b.test/", "B", NOW)]);
  fireEvent.click(screen.getAllByRole("button", { name: "Remove from history" })[0]);
  expect(fetch).toHaveBeenLastCalledWith(...command("history/remove/https%3A%2F%2Fa.test%2F"));
  expect(screen.queryByText("A")).toBeNull();
  expect(screen.getByText("B")).toBeTruthy();
});

test("asks for more pages when the first ones fill the list", () => {
  render(<HistoryList query="" now={NOW} onOpen={() => {}} />);
  const full = Array.from({ length: PAGE_SIZE }, (_, i) =>
    visit(`https://site${i}.test/`, `Site ${i}`, NOW - i),
  );
  answer("", full);
  fireEvent.click(screen.getByRole("button", { name: "Show more" }));
  expect(fetch).toHaveBeenLastCalledWith(...command(`history/search/${2 * PAGE_SIZE}/`));
});

test("says when there is no history", () => {
  render(<HistoryList query="" now={NOW} onOpen={() => {}} />);
  answer("", []);
  expect(screen.getByText("No history yet")).toBeTruthy();
});
