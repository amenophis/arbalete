import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Space } from "./bridge";
import { Spaces } from "./Spaces";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const spaces: Space[] = [
  { id: 13, title: "Home", emoji: null, color: "purple" },
  { id: 14, title: "Work", emoji: "💼", color: "blue" },
  { id: 15, title: "😀 fun", emoji: null, color: "green" },
];

const icon = (title: string) => screen.getByRole("button", { name: title });
const emojiField = () => screen.getByRole("textbox", { name: "Space emoji" }) as HTMLInputElement;
const nameField = () => screen.getByRole("textbox", { name: "Space name" }) as HTMLInputElement;

function edit(displayed = 14) {
  render(<Spaces spaces={spaces} displayed={displayed} />);
  fireEvent.click(icon(spaces.find((space) => space.id === displayed)?.title ?? ""));
}

test("shows each Space's emoji, or the whole first character of its title", () => {
  render(<Spaces spaces={spaces} displayed={13} />);
  const icons = within(screen.getByRole("list", { name: "Spaces" })).getAllByRole("button");
  expect(icons.map((button) => button.textContent)).toEqual(["H", "💼", "😀", "+"]);
});

test("creates a Space", () => {
  render(<Spaces spaces={spaces} displayed={13} />);
  fireEvent.click(icon("New Space"));
  expect(fetch).toHaveBeenLastCalledWith("/command/space/new", { method: "POST" });
});

test("switches to another Space, and opens the displayed one's editor", () => {
  render(<Spaces spaces={spaces} displayed={13} />);
  fireEvent.click(icon("Work"));
  expect(fetch).toHaveBeenLastCalledWith("/command/space/14", { method: "POST" });
  expect(screen.queryByRole("textbox")).toBeNull();
  const home = icon("Home");
  expect(home.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(home);
  expect(home.getAttribute("aria-expanded")).toBe("true");
  expect(nameField().value).toBe("Home");
  expect(fetch).toHaveBeenCalledTimes(1);
  fireEvent.click(home);
  expect(screen.queryByRole("textbox")).toBeNull();
});

test("renames the displayed Space with Enter", () => {
  edit();
  const input = nameField();
  input.focus();
  fireEvent.change(input, { target: { value: " Café/2 " } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(fetch).toHaveBeenLastCalledWith("/command/space/rename/14/Caf%C3%A9%2F2", {
    method: "POST",
  });
});

test("keeps only the first character of the emoji, and removes it when emptied", () => {
  edit(13);
  const input = emojiField();
  expect(input.placeholder).toBe("H");
  fireEvent.change(input, { target: { value: " 👨‍👩‍👧x " } });
  fireEvent.blur(input);
  expect(input.value).toBe("👨‍👩‍👧");
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/space/emoji/13/${encodeURIComponent("👨‍👩‍👧")}`,
    { method: "POST" },
  );
  cleanup();
  fetch.mockClear();
  edit(14);
  fireEvent.change(emojiField(), { target: { value: " " } });
  fireEvent.blur(emojiField());
  expect(fetch).toHaveBeenLastCalledWith("/command/space/emoji/14/", { method: "POST" });
});

test("keeps the emoji on Escape", () => {
  edit();
  const input = emojiField();
  fireEvent.change(input, { target: { value: "🏡" } });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(input.value).toBe("💼");
  expect(fetch).toHaveBeenCalledTimes(0);
});

test("picks the displayed Space's color", () => {
  edit();
  const swatches = screen.getByRole("group", { name: "Color of Work" });
  expect(within(swatches).getByRole("button", { name: "blue" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  fireEvent.click(within(swatches).getByRole("button", { name: "red" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/space/color/14/red", { method: "POST" });
});

test("opens the editor on the name of the Space the app has just created", () => {
  render(<Spaces spaces={spaces} displayed={15} />);
  act(() => {
    dispatchEvent(new CustomEvent("arbalette:rename-space", { detail: 15 }));
  });
  expect(document.activeElement).toBe(nameField());
  fireEvent.blur(nameField());
  expect(nameField().value).toBe("😀 fun");
});
