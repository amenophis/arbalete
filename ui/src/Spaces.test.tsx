import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Space } from "./bridge";
import { NewSpace, Spaces } from "./Spaces";

const fetch = vi.fn();
const onPage = vi.fn();

beforeEach(() => {
  onPage.mockReset();
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
  render(<Spaces spaces={spaces} displayed={displayed} onPage={onPage} />);
  fireEvent.click(icon(spaces.find((space) => space.id === displayed)?.title ?? ""));
}

test("shows each Space's emoji, or a dot", () => {
  render(<Spaces spaces={spaces} displayed={13} onPage={onPage} />);
  const icons = within(screen.getByRole("list", { name: "Spaces" })).getAllByRole("button");
  expect(icons.map((button) => button.textContent)).toEqual(["", "💼", ""]);
  expect(icons[0].querySelector(".dot")).not.toBeNull();
});

test("goes to another Space's page, and opens the displayed one's editor", () => {
  render(<Spaces spaces={spaces} displayed={13} onPage={onPage} />);
  fireEvent.click(icon("Work"));
  expect(onPage).toHaveBeenLastCalledWith(1);
  expect(screen.queryByRole("textbox")).toBeNull();
  const home = icon("Home");
  expect(home.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(home);
  expect(home.getAttribute("aria-expanded")).toBe("true");
  expect(nameField().value).toBe("Home");
  expect(onPage).toHaveBeenCalledTimes(1);
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
