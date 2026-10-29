import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { PinnedExtensions, Site } from "./Site";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

function show(url: string) {
  const extensions = [
    {
      id: "dark",
      name: "Dark Reader",
      icon: "arbalete://ui/extension-icon/1/dark",
      popup: true,
      enabled: true,
      pinned: true,
    },
    { id: "quiet", name: "No Popup", icon: null, popup: false, enabled: true, pinned: false },
    { id: "off", name: "Off", icon: null, popup: true, enabled: false, pinned: false },
  ];
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:show", { detail: { url, extensions } }));
  });
}

test("acts on the site and opens an extension's popup", () => {
  render(<Site />);
  show("https://example.com/");
  expect(fetch).toHaveBeenCalledWith(expect.stringMatching(/^\/command\/site\/height\//), {
    method: "POST",
  });
  expect(screen.getByText("Secure")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Clear Cookies" }));
  expect(fetch).toHaveBeenCalledWith("/command/site/hide", { method: "POST" });
  expect(fetch).toHaveBeenLastCalledWith("/command/menu/choose/site/any/clear-cookies", {
    method: "POST",
  });
  expect(screen.getByRole("button", { name: "No Popup" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "Off" })).toHaveProperty("disabled", true);
  expect(screen.getAllByLabelText("Pinned")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Dark Reader" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/site/extension/dark", { method: "POST" });
  fireEvent.click(screen.getByRole("button", { name: "Add Extensions" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/site/store", { method: "POST" });
});

test("closes on Escape and says when a site is not secure", () => {
  render(<Site />);
  show("http://example.com/");
  expect(screen.getByText("Not Secure")).toBeTruthy();
  fireEvent.keyDown(window, { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/site/hide", { method: "POST" });
});

test("opens a pinned extension's popup under its button", () => {
  const extensions = [{ id: "dark", name: "Dark Reader", icon: null, popup: true }];
  render(<PinnedExtensions extensions={extensions} view="toolbar" />);
  const button = screen.getByRole("button", { name: "Dark Reader" });
  button.getBoundingClientRect = () => ({ x: 10.4, y: 4, width: 26, height: 25.6 }) as DOMRect;
  fireEvent.click(button);
  expect(fetch).toHaveBeenLastCalledWith("/command/extension/popup/dark/toolbar/10/4/26/26", {
    method: "POST",
  });
});
