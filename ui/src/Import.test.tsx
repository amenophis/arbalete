import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Import, type Sources } from "./Import";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const sources: Sources = {
  browsers: [
    {
      name: "Arc",
      arc: true,
      profiles: [
        { name: "Personal", needsFullDiskAccess: false },
        { name: "Work", needsFullDiskAccess: false },
      ],
    },
    { name: "Chrome", arc: false, profiles: [{ name: "Default", needsFullDiskAccess: false }] },
    { name: "Safari", arc: false, profiles: [{ name: "Default", needsFullDiskAccess: true }] },
  ],
  profiles: [],
};

function receive(detail: Sources) {
  act(() => {
    dispatchEvent(new CustomEvent("arbalette:sources", { detail }));
  });
}

const lastRun = () => {
  const url: string = fetch.mock.lastCall?.[0];
  return JSON.parse(decodeURIComponent(url.replace("/command/import/run/", "")));
};

test("asks the app for the browsers found", () => {
  render(<Import />);
  expect(fetch).toHaveBeenCalledWith("/command/import/ready", { method: "POST" });
});

test("imports Arc's profiles by default, each into a new profile", () => {
  render(<Import />);
  receive(sources);
  expect(screen.getByRole("checkbox", { name: "Import Arc Personal" })).toHaveProperty(
    "checked",
    true,
  );
  expect(screen.getByRole("checkbox", { name: "Import Chrome Default" })).toHaveProperty(
    "checked",
    false,
  );
  // Nothing to merge into on the first launch, and no bookmarks bar in Arc.
  expect(screen.queryByRole("combobox", { name: /Destination/ })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "Bookmarks bar of Arc Work" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Import" }));
  expect(lastRun()).toEqual([
    { browser: 0, profile: 0, into: null, bar: "favorites" },
    { browser: 0, profile: 1, into: null, bar: "favorites" },
  ]);
});

test("merges a profile into an existing one, with its bar as a pinned folder", () => {
  render(<Import />);
  receive({ ...sources, profiles: [{ id: 7, name: "Default" }] });
  fireEvent.click(screen.getByRole("checkbox", { name: "Import Arc Personal" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Import Arc Work" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Import Chrome Default" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Destination of Chrome Default" }), {
    target: { value: "7" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "Bookmarks bar of Chrome Default" }), {
    target: { value: "pinnedFolder" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Import" }));
  expect(lastRun()).toEqual([{ browser: 1, profile: 0, into: 7, bar: "pinnedFolder" }]);
});

test("imports nothing until a profile is picked", () => {
  render(<Import />);
  receive({ ...sources, browsers: [sources.browsers[1]] });
  expect(screen.getByRole("button", { name: "Import" })).toHaveProperty("disabled", true);
});

test("explains Full Disk Access and opens its settings", () => {
  render(<Import />);
  receive(sources);
  expect(screen.queryByRole("checkbox", { name: "Import Safari Default" })).toBeNull();
  expect(screen.getByText(/Full Disk Access/)).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "Open System Settings" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/import/full-disk-access", {
    method: "POST",
  });
});

test("closes without importing", () => {
  render(<Import />);
  receive(sources);
  fireEvent.click(screen.getByRole("button", { name: "Not now" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/import/close", { method: "POST" });
  fetch.mockClear();
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/import/close", { method: "POST" });
});

test("starts over when the app sends the browsers again", () => {
  render(<Import />);
  receive(sources);
  fireEvent.click(screen.getByRole("checkbox", { name: "Import Arc Work" }));
  receive(sources);
  expect(screen.getByRole("checkbox", { name: "Import Arc Work" })).toHaveProperty("checked", true);
});
