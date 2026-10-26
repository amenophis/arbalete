import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Find } from "./Find";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const command = (path: string) => [`/command/${path}`, { method: "POST" }];

test("searches as the text changes, and goes to the next or previous match on enter", () => {
  render(<Find />);
  const input = screen.getByRole("textbox", { name: "Find in page" });
  fireEvent.change(input, { target: { value: "café" } });
  expect(fetch).toHaveBeenLastCalledWith(...command("find/forward/caf%C3%A9"));
  fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  expect(fetch).toHaveBeenLastCalledWith(...command("find/backward/caf%C3%A9"));
  fireEvent.click(screen.getByRole("button", { name: "Next match" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("find/forward/caf%C3%A9"));
});

test("shows the active match and the count", () => {
  render(<Find />);
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:found", { detail: { count: 12, active: 3 } }));
  });
  expect(screen.getByText("3/12")).toBeTruthy();
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:found", { detail: { count: 12, active: 0 } }));
  });
  expect(screen.getByText("–/12")).toBeTruthy();
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:found", { detail: { count: 0, active: 0 } }));
  });
  expect(screen.getByText("No results")).toBeTruthy();
});

test("selects the text last searched for when shown", () => {
  render(<Find />);
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:show", { detail: "word" }));
  });
  const input = screen.getByRole<HTMLInputElement>("textbox");
  expect(input.value).toBe("word");
  expect([input.selectionStart, input.selectionEnd]).toEqual([0, 4]);
});

test("hides on escape and from its close button", () => {
  render(<Find />);
  fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith(...command("find/hide"));
  fetch.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(fetch).toHaveBeenLastCalledWith(...command("find/hide"));
});
