import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Site } from "./Site";

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
      icon: "https://arbalete.ui/extension-icon/1/dark",
      popup: true,
    },
    { id: "quiet", name: "No Popup", icon: null, popup: false },
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
  fireEvent.click(screen.getByRole("button", { name: "Dark Reader" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/site/extension/dark", { method: "POST" });
});

test("closes on Escape and says when a site is not secure", () => {
  render(<Site />);
  show("http://example.com/");
  expect(screen.getByText("Not Secure")).toBeTruthy();
  fireEvent.keyDown(window, { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/site/hide", { method: "POST" });
});
