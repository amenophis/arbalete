import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Little, type LittleDetail } from "./Little";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

function receive(url: string) {
  const detail: LittleDetail = {
    url,
    title: "",
    space: { title: "Work", emoji: "💼" },
    search: { name: "DuckDuckGo", url: "https://duckduckgo.com/?q=" },
  };
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:little", { detail }));
  });
}

const field = () => screen.getByRole("textbox", { name: "Search or enter URL" });

test("asks for its page once it listens, and types an address while there is none", () => {
  render(<Little id={4} />);
  expect(fetch).toHaveBeenCalledWith("/command/little/4/ready", { method: "POST" });
  receive("about:blank");
  expect(document.activeElement).toBe(field());
  expect(screen.getByRole("button", { name: /Open in/ })).toHaveProperty("disabled", true);
  fireEvent.change(field(), { target: { value: "example.com" } });
  fireEvent.keyDown(field(), { key: "Enter" });
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/little/4/navigate/${encodeURIComponent("https://example.com")}`,
    { method: "POST" },
  );
});

test("searches with its profile's engine what is not an address", () => {
  render(<Little id={4} />);
  receive("");
  fireEvent.change(field(), { target: { value: "arc browser" } });
  fireEvent.keyDown(field(), { key: "Enter" });
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/little/4/navigate/${encodeURIComponent("https://duckduckgo.com/?q=arc%20browser")}`,
    { method: "POST" },
  );
});

test("shows its page's domain, which a click edits, and moves the page into the Space", () => {
  render(<Little id={4} />);
  receive("https://www.example.com/page");
  const domain = screen.getByRole("button", { name: "example.com" });
  expect(domain.getAttribute("title")).toBe("https://www.example.com/page");
  fireEvent.click(domain);
  expect((field() as HTMLInputElement).value).toBe("https://www.example.com/page");
  fireEvent.keyDown(field(), { key: "Escape" });
  expect(screen.queryByRole("textbox")).toBeNull();
  const open = screen.getByRole("button", { name: /Open in/ });
  expect(open.textContent).toBe("Open in 💼 Work⌘O");
  fireEvent.click(open);
  expect(fetch).toHaveBeenLastCalledWith("/command/little/4/open", { method: "POST" });
});
