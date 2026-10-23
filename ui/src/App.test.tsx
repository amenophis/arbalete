import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { App } from "./App";
import type { Page } from "./bridge";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

function receive(page: Partial<Page>) {
  const detail = {
    page: {
      url: "https://example.com/",
      title: "Example",
      favicon: null,
      progress: 1,
      loading: false,
      canGoBack: true,
      canGoForward: true,
      ...page,
    },
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

test("shows the page and its loading progress", () => {
  render(<App />);
  receive({ title: "Wikipedia", loading: true, progress: 0.4, canGoBack: false });
  expect(screen.getByText("Wikipedia")).toBeDefined();
  expect(screen.getByRole("progressbar", { name: "Loading" })).toBeDefined();
  expect(screen.getByRole("button", { name: "Back" })).toHaveProperty("disabled", true);
});
