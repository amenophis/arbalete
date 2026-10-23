import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Overlay, toUrl } from "./Overlay";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

test("opens the typed address in a tab", () => {
  render(<Overlay />);
  const input = screen.getByRole("textbox", { name: "Open a URL" });
  fireEvent.change(input, { target: { value: "example.com/été" } });
  fireEvent.submit(input);
  expect(fetch).toHaveBeenLastCalledWith(
    "/command/open/https%3A%2F%2Fexample.com%2F%C3%A9t%C3%A9",
    {
      method: "POST",
    },
  );
});

test("hides on escape", () => {
  render(<Overlay />);
  fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/overlay/hide", { method: "POST" });
});

test("keeps urls that have a scheme", () => {
  expect(toUrl("about:blank")).toBe("about:blank");
  expect(toUrl("http://example.com")).toBe("http://example.com");
  expect(toUrl("example.com")).toBe("https://example.com");
});
