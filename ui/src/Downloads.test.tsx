import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { type Download, Downloads, detail } from "./Downloads";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const command = (path: string) => [`/command/${path}`, { method: "POST" }];

const download = (changes: Partial<Download>): Download => ({
  id: 1,
  name: "report.pdf",
  received: 0,
  total: 0,
  speed: 0,
  state: "inProgress",
  ...changes,
});

function show(downloads: Download[]) {
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:downloads", { detail: downloads }));
  });
}

test("shows nothing without downloads", () => {
  const { container } = render(<Downloads />);
  show([]);
  expect(container.innerHTML).toBe("");
});

test("cancels a download in progress", () => {
  render(<Downloads />);
  show([download({ id: 4, received: 5e6, total: 20e6, speed: 1e6 })]);
  expect(screen.getByText("5.0 / 20.0 MB · 15 s left")).toBeTruthy();
  expect(screen.getByRole("button", { name: /report\.pdf/ })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel download" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/cancel/4"));
});

test("opens a finished download, shows it in the Finder and takes it off the list", () => {
  render(<Downloads />);
  show([download({ id: 2, received: 2048, total: 2048, state: "complete" })]);
  expect(screen.getByText("2.0 KB")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: /report\.pdf/ }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/open/2"));
  fireEvent.click(screen.getByRole("button", { name: "Show in Finder" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/show/2"));
  fireEvent.click(screen.getByRole("button", { name: "Remove from list" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("download/remove/2"));
});

test("describes each state", () => {
  expect(detail(download({ received: 512 }))).toBe("512 B");
  expect(detail(download({ received: 3e6, total: 9e6 }))).toBe("3.0 / 9.0 MB");
  expect(detail(download({ received: 0, total: 3.6e9, speed: 1e6 }))).toBe(
    "0.0 / 3.6 GB · 1 h left",
  );
  expect(detail(download({ state: "canceled" }))).toBe("Canceled");
  expect(detail(download({ state: "failed" }))).toBe("Failed");
});
