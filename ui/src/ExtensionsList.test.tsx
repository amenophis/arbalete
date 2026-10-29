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

test("manages each extension of the profile", () => {
  render(<ExtensionsList extensions={extensions} query="" />);
  fireEvent.click(screen.getByRole("switch", { name: "Turn Dark Reader on" }));
  expect(sent()).toBe("/command/extension/off/dark");
  fireEvent.click(screen.getByRole("switch", { name: "Turn uBlock Origin Lite on" }));
  expect(sent()).toBe("/command/extension/on/block");
  fireEvent.click(screen.getByRole("button", { name: "Unpin Dark Reader" }));
  expect(sent()).toBe("/command/extension/unpin/dark");
  expect(screen.queryByRole("button", { name: "Dark Reader options" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "uBlock Origin Lite options" }));
  expect(sent()).toBe("/command/extension/options/block");
  fireEvent.click(screen.getByRole("button", { name: "Remove uBlock Origin Lite" }));
  expect(sent()).toBe("/command/extension/remove/block");
  fireEvent.click(screen.getByRole("button", { name: "Load Unpacked…" }));
  expect(sent()).toBe("/command/extension/load-unpacked");
});

test("searches the extensions by name and description", () => {
  render(<ExtensionsList extensions={extensions} query="blocker" />);
  expect(screen.queryByText("Dark Reader")).toBeNull();
  expect(screen.getByText("uBlock Origin Lite")).toBeTruthy();
});

test("details an extension like Chromium's extensions page", () => {
  render(<ExtensionsList extensions={extensions} query="" />);
  const row = screen.getByRole("button", { name: /^Dark Reader 4/ });
  expect(row.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(row);
  expect(screen.getByText("dark")).toBeTruthy();
  expect(screen.getByText("4.9 (Manifest V3)")).toBeTruthy();
  expect(screen.getByText("Oct 28, 2026")).toBeTruthy();
  expect(screen.getByText("storage")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Chrome Web Store" }));
  expect(sent()).toBe(
    `/command/open/${encodeURIComponent("https://chromewebstore.google.com/detail/dark")}`,
  );
  fireEvent.click(screen.getByRole("button", { name: /^uBlock Origin Lite 2026/ }));
  expect(screen.queryByText("4.9 (Manifest V3)")).toBeNull();
  expect(screen.getByText("Unpacked, from /Users/me/block")).toBeTruthy();
  expect(screen.getAllByText("None")).toHaveLength(2);
});
