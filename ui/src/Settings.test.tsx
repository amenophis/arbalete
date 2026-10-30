import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Settings, type SettingsDetail, SHORTCUTS } from "./Settings";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const detail: SettingsDetail = {
  settings: {
    warnBeforeQuit: false,
    restoreSession: true,
    suspendTabs: true,
    awakeSites: ["mail.google.com"],
  },
  engines: [
    { id: "google", name: "Google" },
    { id: "duckduckgo", name: "DuckDuckGo" },
  ],
  delays: [
    { id: "twelve-hours", name: "12 hours" },
    { id: "never", name: "Never" },
  ],
  profiles: [
    {
      id: 1,
      name: "Default",
      color: "purple",
      searchEngine: "google",
      searchSuggestions: true,
      archiveAfter: "twelve-hours",
    },
    {
      id: 5,
      name: "Work",
      color: "blue",
      searchEngine: "duckduckgo",
      searchSuggestions: true,
      archiveAfter: "never",
    },
  ],
};

function open() {
  render(<Settings />);
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:settings", { detail }));
  });
}

test("asks the app for the settings once it listens, and opens on General", () => {
  render(<Settings />);
  expect(fetch).toHaveBeenCalledWith("/command/settings/ready", { method: "POST" });
  expect(screen.queryByRole("tablist")).toBeNull();
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:settings", { detail }));
  });
  const tabs = screen.getAllByRole("tab").map((tab) => tab.textContent);
  expect(tabs).toEqual(["General", "Profiles", "Shortcuts"]);
  expect(screen.getByRole("tabpanel").getAttribute("aria-label")).toBe("General");
  // The window's title is the picked section's name.
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("General");
  fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Shortcuts");
});

test("General turns the app's settings on or off", () => {
  open();
  const warn = screen.getByRole("checkbox", { name: /Warn before quitting/ });
  const restore = screen.getByRole("checkbox", { name: /Restore the previous session/ });
  expect([(warn as HTMLInputElement).checked, (restore as HTMLInputElement).checked]).toEqual([
    false,
    true,
  ]);
  fireEvent.click(warn);
  expect(fetch).toHaveBeenLastCalledWith("/command/settings/warn-before-quit/true", {
    method: "POST",
  });
  fireEvent.click(restore);
  expect(fetch).toHaveBeenLastCalledWith("/command/settings/restore-session/false", {
    method: "POST",
  });
});

test("General suspends tabs left aside, but for the sites kept awake", () => {
  open();
  fireEvent.click(screen.getByRole("button", { name: "Let mail.google.com sleep" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/awake/off/mail.google.com", { method: "POST" });
  fireEvent.click(screen.getByRole("checkbox", { name: /Save memory with tabs left aside/ }));
  expect(fetch).toHaveBeenLastCalledWith("/command/settings/suspend-tabs/false", {
    method: "POST",
  });
});

test("Profiles picks each profile's search engine", () => {
  open();
  fireEvent.click(screen.getByRole("tab", { name: "Profiles" }));
  const work = screen.getByRole("combobox", { name: "Work search engine" }) as HTMLSelectElement;
  expect(work.value).toBe("duckduckgo");
  fireEvent.change(screen.getByRole("combobox", { name: "Default search engine" }), {
    target: { value: "duckduckgo" },
  });
  expect(fetch).toHaveBeenLastCalledWith("/command/profile/search/1/duckduckgo", {
    method: "POST",
  });
});

test("Profiles picks how long each profile's tabs stay before they are archived", () => {
  open();
  fireEvent.click(screen.getByRole("tab", { name: "Profiles" }));
  const work = screen.getByRole("combobox", {
    name: "Work archives tabs after",
  }) as HTMLSelectElement;
  expect(work.value).toBe("never");
  fireEvent.change(screen.getByRole("combobox", { name: "Default archives tabs after" }), {
    target: { value: "never" },
  });
  expect(fetch).toHaveBeenLastCalledWith("/command/profile/archive/1/never", { method: "POST" });
});

test("Shortcuts lists every shortcut by group", () => {
  open();
  fireEvent.click(screen.getByRole("tab", { name: "Shortcuts" }));
  const groups = screen.getAllByRole("region").map((group) => group.getAttribute("aria-label"));
  expect(groups).toEqual(SHORTCUTS.map(([group]) => group));
  const tabs = within(screen.getByRole("region", { name: "Tabs" }));
  expect(tabs.getByText("Switch Between Recent Tabs").nextSibling?.textContent).toBe("⌃⇥, ⌃⇧⇥");
});

test("Escape closes the settings", () => {
  open();
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/settings/hide", { method: "POST" });
});
