import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { ExtensionsList } from "./ExtensionsList";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const extensions = [
  {
    id: "dark",
    name: "Dark Reader",
    icon: null,
    popup: true,
    enabled: true,
    pinned: true,
    options: false,
    description: "Dark mode for every website",
    version: "4.9",
    details: {
      permissions: ["storage"],
      sites: ["<all_urls>"],
      homepage: null,
      manifestVersion: 3,
      fromStore: true,
      unpacked: null,
      installedAt: Date.UTC(2026, 9, 28),
      updatedAt: Date.UTC(2026, 9, 28),
    },
  },
  {
    id: "block",
    name: "uBlock Origin Lite",
    icon: null,
    popup: true,
    enabled: false,
    pinned: false,
    options: true,
    description: "A content blocker",
    version: "2026.1",
    details: {
      permissions: [],
      sites: [],
      homepage: "https://example.org/",
      manifestVersion: 3,
      fromStore: false,
      unpacked: "/Users/me/block",
      installedAt: null,
      updatedAt: null,
    },
  },
];

const sent = () => fetch.mock.lastCall?.[0];

test("lists the extensions, the first one detailed beside them", () => {
  render(<ExtensionsList extensions={extensions} query="" />);
  const details = screen.getByRole("region", { name: "Dark Reader details" });
  expect(details.textContent).toContain("4.9 · Manifest V3");
  expect(details.textContent).toContain("Oct 28, 2026");
  expect(details.textContent).toContain("storage");
  expect(details.textContent).toContain("All sites");
  fireEvent.click(screen.getByRole("button", { name: "Web Store" }));
  expect(sent()).toBe(
    `/command/open/${encodeURIComponent("https://chromewebstore.google.com/detail/dark")}`,
  );
  fireEvent.click(screen.getByRole("button", { name: "Unpin" }));
  expect(sent()).toBe("/command/extension/unpin/dark");
  expect(screen.queryByRole("button", { name: "Options" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  expect(sent()).toBe("/command/extension/remove/dark");
});

test("details the extension picked, and turns each one on or off", () => {
  render(<ExtensionsList extensions={extensions} query="" />);
  fireEvent.click(screen.getByRole("button", { name: "uBlock Origin Lite" }));
  const details = screen.getByRole("region", { name: "uBlock Origin Lite details" });
  expect(details.textContent).toContain("Unpacked, from /Users/me/block");
  fireEvent.click(screen.getByRole("button", { name: "Options" }));
  expect(sent()).toBe("/command/extension/options/block");
  fireEvent.click(screen.getByRole("button", { name: "Website" }));
  expect(sent()).toBe(`/command/open/${encodeURIComponent("https://example.org/")}`);
  const switches = screen.getAllByRole("switch", { name: "Turn uBlock Origin Lite on" });
  fireEvent.click(switches[0]);
  expect(sent()).toBe("/command/extension/on/block");
  fireEvent.click(screen.getAllByRole("switch", { name: "Turn Dark Reader on" })[0]);
  expect(sent()).toBe("/command/extension/off/dark");
  fireEvent.click(screen.getByRole("button", { name: "Load Unpacked…" }));
  expect(sent()).toBe("/command/extension/load-unpacked");
});

test("searches the extensions by name and description", () => {
  render(<ExtensionsList extensions={extensions} query="blocker" />);
  expect(screen.queryByRole("button", { name: /Dark Reader/ })).toBeNull();
  expect(screen.getByRole("region", { name: "uBlock Origin Lite details" })).toBeTruthy();
});
