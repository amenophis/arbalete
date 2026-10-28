import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Toolbar } from "./Toolbar";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

function receive(loading: boolean) {
  const tab = { id: 3, url: "https://www.example.com/page", title: "Example", favicon: null };
  const detail = {
    page: { progress: 0.4, loading, canGoBack: true, canGoForward: false },
    space: 13,
    spaces: [],
    pages: [{ space: 13, favorites: [], pinned: [], today: [tab], selected: 3, collapsed: false }],
    profiles: [],
    closed: [],
    toolbar: true,
  };
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:state", { detail }));
  });
}

test("navigates the page and shows its domain, which a click edits", () => {
  render(<Toolbar />);
  expect(fetch).toHaveBeenCalledWith("/command/ready", { method: "POST" });
  receive(false);
  expect(screen.getByRole("button", { name: "Back" })).toHaveProperty("disabled", false);
  expect(screen.getByRole("button", { name: "Forward" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "Reload" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/reload", { method: "POST" });
  const url = screen.getByRole("button", { name: "example.com" });
  expect(url.getAttribute("title")).toBe("https://www.example.com/page");
  fireEvent.click(url);
  expect(fetch).toHaveBeenLastCalledWith("/command/overlay/edit", { method: "POST" });
  fireEvent.click(screen.getByRole("button", { name: "Site controls" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/site/show/toolbar/0/0/0/0", { method: "POST" });
  expect(screen.queryByRole("progressbar")).toBeNull();
  receive(true);
  expect(screen.getByRole("progressbar", { name: "Loading" })).toBeDefined();
});
