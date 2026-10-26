import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import {
  type Download,
  Downloads,
  DownloadsList,
  day,
  detail,
  RECENT_MS,
  useDownloads,
} from "./Downloads";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const command = (path: string) => [`/command/${path}`, { method: "POST" }];

const NOW = new Date(2026, 9, 26, 15, 0).getTime();

const download = (changes: Partial<Download>): Download => ({
  id: 1,
  name: "report.pdf",
  url: "https://www.example.com/files/report.pdf",
  received: 0,
  total: 0,
  speed: 0,
  state: "inProgress",
  startedAt: NOW - 60_000,
  missing: false,
  ...changes,
});

test("follows the downloads the app sends", () => {
  const { result } = renderHook(() => useDownloads());
  const list = [download({ id: 3 })];
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:downloads", { detail: list }));
  });
  expect(result.current).toEqual(list);
});

test("shows the downloads in progress and the last day's, three at most, in the sidebar", () => {
  const old = NOW - RECENT_MS - 1;
  const { container } = render(
    <Downloads
      now={NOW}
      downloads={[
        download({ id: 1, name: "old.zip", state: "complete", startedAt: old }),
        download({ id: 2, name: "long.iso", startedAt: old }),
        download({ id: 3, name: "a.pdf", state: "complete" }),
        download({ id: 4, name: "b.pdf", state: "complete" }),
        download({ id: 5, name: "c.pdf", state: "complete" }),
      ]}
    />,
  );
  const names = [...container.querySelectorAll(".title")].map((title) => title.textContent);
  expect(names).toEqual(["a.pdf", "b.pdf", "c.pdf"]);
  cleanup();
  const { container: empty } = render(<Downloads now={NOW} downloads={[]} />);
  expect(empty.innerHTML).toBe("");
});

test("cancels a download in progress", () => {
  render(
    <Downloads
      now={NOW}
      downloads={[download({ id: 4, received: 5e6, total: 20e6, speed: 1e6 })]}
    />,
  );
  expect(screen.getByText("5.0 / 20.0 MB · 15 s left")).toBeTruthy();
  expect(screen.getByRole("button", { name: /report\.pdf/ })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel download" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/cancel/4"));
});

test("opens a finished download, shows it in the Finder and takes it off the list", () => {
  render(
    <Downloads
      now={NOW}
      downloads={[download({ id: 2, received: 2048, total: 2048, state: "complete" })]}
    />,
  );
  expect(screen.getByText("2.0 KB")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /report\.pdf/ }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/open/2"));
  fireEvent.click(screen.getByRole("button", { name: "Show in Finder" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/show/2"));
  fireEvent.click(screen.getByRole("button", { name: "Remove from list" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/remove/2"));
});

test("lists every download by day, the most recent first, with its site, and searches them", () => {
  const yesterday = NOW - 24 * 60 * 60 * 1000;
  const list = [
    download({ id: 1, name: "old.zip", state: "complete", received: 5e6, startedAt: yesterday }),
    download({ id: 2, name: "gone.pdf", state: "complete", missing: true }),
    download({ id: 3, name: "new.pdf", state: "failed", url: "https://files.test/new.pdf" }),
  ];
  const { container, rerender } = render(<DownloadsList downloads={list} query="" now={NOW} />);
  const headings = [...container.querySelectorAll("h3")].map((heading) => heading.textContent);
  expect(headings).toEqual(["Today", "Yesterday"]);
  const names = [...container.querySelectorAll(".title")].map((title) => title.textContent);
  expect(names).toEqual(["new.pdf", "gone.pdf", "old.zip"]);
  expect(screen.getByText("Failed · files.test")).toBeTruthy();
  expect(screen.getByText("Deleted · example.com")).toBeTruthy();
  expect(screen.getByRole("button", { name: /gone\.pdf/ })).toHaveProperty("disabled", true);
  rerender(<DownloadsList downloads={list} query="files.test" now={NOW} />);
  expect(container.querySelectorAll(".title")).toHaveLength(1);
  rerender(<DownloadsList downloads={list} query="nothing" now={NOW} />);
  expect(screen.getByText("No download matches")).toBeTruthy();
  rerender(<DownloadsList downloads={[]} query="" now={NOW} />);
  expect(screen.getByText("No download yet")).toBeTruthy();
});

test("describes each state and day", () => {
  expect(detail(download({ received: 512 }))).toBe("512 B");
  expect(detail(download({ received: 3e6, total: 9e6 }))).toBe("3.0 / 9.0 MB");
  expect(detail(download({ received: 0, total: 3.6e9, speed: 1e6 }))).toBe(
    "0.0 / 3.6 GB · 1 h left",
  );
  expect(detail(download({ state: "canceled" }))).toBe("Canceled");
  expect(detail(download({ state: "failed" }))).toBe("Failed");
  expect(day(NOW - 3_600_000, NOW)).toBe("Today");
  expect(day(new Date(2026, 9, 25, 23, 0).getTime(), NOW)).toBe("Yesterday");
  expect(day(new Date(2026, 9, 20).getTime(), NOW)).toBe(
    new Date(2026, 9, 20).toLocaleDateString(undefined, { dateStyle: "long" }),
  );
});
