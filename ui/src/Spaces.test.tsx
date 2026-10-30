import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Space } from "./bridge";
import { NewSpace, Spaces } from "./Spaces";
import type { Theme } from "./theme";

// A theme of one color, like a Space saved before themes.
const solid = (color: string): Theme => ({ colors: [color], mode: "auto", intensity: 1, noise: 0 });

const fetch = vi.fn();
const onPage = vi.fn();

beforeEach(() => {
  onPage.mockReset();
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const spaces: Space[] = [
  { id: 13, title: "Home", emoji: null, theme: solid("#6e56cf"), profile: 1, pinned: [] },
  { id: 14, title: "Work", emoji: "💼", theme: solid("#3e7bfa"), profile: 1, pinned: [] },
  { id: 15, title: "😀 fun", emoji: null, theme: solid("#30a46c"), profile: 1, pinned: [] },
];

const icon = (title: string) => screen.getByRole("button", { name: title });
const emojiField = () => screen.getByRole("textbox", { name: "Space emoji" }) as HTMLInputElement;
const nameField = () => screen.getByRole("textbox", { name: "Space name" }) as HTMLInputElement;

// Opens the displayed Space's editor, as its menu does.
function edit(displayed = 14) {
  render(<Spaces spaces={spaces} displayed={displayed} onPage={onPage} />);
  act(() => {
    dispatchEvent(
      new CustomEvent("arbalete:edit-space", { detail: { space: displayed, field: "name" } }),
    );
  });
}

test("shows each Space's emoji, or a dot", () => {
  render(<Spaces spaces={spaces} displayed={13} onPage={onPage} />);
  const icons = within(screen.getByRole("list", { name: "Spaces" })).getAllByRole("button");
  expect(icons.map((button) => button.textContent)).toEqual(["", "💼", ""]);
  expect(icons[0].querySelector(".dot")).not.toBeNull();
});

test("goes to another Space's page, and opens the displayed one's theme picker", () => {
  render(<Spaces spaces={spaces} displayed={13} onPage={onPage} />);
  fireEvent.click(icon("Work"));
  expect(onPage).toHaveBeenLastCalledWith(1);
  expect(fetch).not.toHaveBeenCalled();
  fireEvent.click(icon("Home"));
  expect(fetch).toHaveBeenLastCalledWith("/command/theme/show/13", { method: "POST" });
  expect(screen.queryByRole("textbox")).toBeNull();
  expect(onPage).toHaveBeenCalledTimes(1);
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

test("the Space's menu opens its editor on the field it edits", () => {
  render(<Spaces spaces={spaces} displayed={14} onPage={onPage} />);
  const ask = (space: number, field: string) =>
    act(() => {
      dispatchEvent(new CustomEvent("arbalete:edit-space", { detail: { space, field } }));
    });
  ask(13, "emoji");
  expect(screen.queryByRole("textbox", { name: "Space emoji" })).toBeNull();
  ask(14, "emoji");
  expect(document.activeElement).toBe(emojiField());
  ask(14, "name");
  expect(document.activeElement).toBe(nameField());
  expect(icon("Work").getAttribute("aria-expanded")).toBe("true");
});

test("opens the displayed Space's theme picker from its editor", () => {
  edit();
  fireEvent.click(screen.getByRole("button", { name: "Edit Theme…" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/theme/show/14", { method: "POST" });
});

test("goes back to the displayed Space's page while another page shows", () => {
  render(<Spaces spaces={spaces} onPage={onPage} />);
  const home = icon("Home");
  expect(home.hasAttribute("aria-current")).toBe(false);
  fireEvent.click(home);
  expect(onPage).toHaveBeenLastCalledWith(0);
  expect(screen.queryByRole("textbox")).toBeNull();
});

const field = (name: string) =>
  screen.getByRole("textbox", { name: `New Space ${name}` }) as HTMLInputElement;
const create = () => screen.getByRole("button", { name: "Create" }) as HTMLButtonElement;

test("creates a Space from its name, emoji and color with Enter", () => {
  render(<NewSpace index={3} shown />);
  expect(document.activeElement).toBe(field("name"));
  expect(create().disabled).toBe(true);
  fireEvent.change(field("name"), { target: { value: " Café/2 " } });
  fireEvent.change(field("emoji"), { target: { value: "📚x" } });
  fireEvent.click(
    within(screen.getByRole("group", { name: "Color of the new Space" })).getByRole("button", {
      name: "orange",
    }),
  );
  fireEvent.submit(field("name"));
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/space/create/orange/${encodeURIComponent("📚")}/Caf%C3%A9%2F2`,
    { method: "POST" },
  );
});

test("creates nothing without a name", () => {
  render(<NewSpace index={3} shown />);
  fireEvent.change(field("name"), { target: { value: "  " } });
  expect(create().disabled).toBe(true);
  fireEvent.submit(field("name"));
  expect(fetch).not.toHaveBeenCalled();
});

test("forgets what was typed once left, and stays out of reach meanwhile", () => {
  const { rerender } = render(<NewSpace index={3} shown />);
  fireEvent.change(field("name"), { target: { value: "Reading" } });
  fireEvent.change(field("emoji"), { target: { value: "📚" } });
  rerender(<NewSpace index={3} shown={false} />);
  expect(screen.getByRole("form", { hidden: true }).hasAttribute("inert")).toBe(true);
  rerender(<NewSpace index={3} shown />);
  expect(field("name").value).toBe("");
  expect(field("emoji").value).toBe("");
  expect(fetch).not.toHaveBeenCalled();
});
