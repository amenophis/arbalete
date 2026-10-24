import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { App } from "./App";
import type { Page, Tab } from "./bridge";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const tabs: Tab[] = [
  { id: 3, url: "https://example.com/", title: "Example", favicon: null },
  { id: 4, url: "https://example.org/", title: "Other", favicon: null },
];

function receive(page: Partial<Page>, active = 3) {
  const detail = {
    page: { progress: 1, loading: false, canGoBack: true, canGoForward: true, ...page },
    tabs,
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
