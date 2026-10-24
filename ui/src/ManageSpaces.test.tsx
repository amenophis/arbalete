import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Space } from "./bridge";
import { ManageSpaces, matches, RENAME_CLICK_MS, spaceMoveCommand } from "./ManageSpaces";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const spaces: Space[] = [
  { id: 13, title: "Home", emoji: "🏠", color: "blue", profile: 1, pinned: [] },
  {
    id: 14,
    title: "Work",
    emoji: null,
    color: "green",
    profile: 7,
    pinned: [
      { id: 3, url: "https://mail.test/", title: "Mail", favicon: "https://ui.test/mail.png" },
      { id: 4, title: "Projects", folder: true },
      { id: 5, url: "https://untitled.test/", title: "", favicon: null },
    ],
  },
  { id: 15, title: "Play", emoji: null, color: "red", profile: 1, pinned: [] },
];

const profiles = [
  { id: 1, name: "Default", color: "purple" as const },
  { id: 7, name: "Office", color: "blue" as const },
];

function renderCards(onShow = vi.fn(), onNewSpace = vi.fn(), query = "") {
  render(
    <ManageSpaces
      spaces={spaces}
      profiles={profiles}
      query={query}
      onShow={onShow}
      onNewSpace={onNewSpace}
    />,
  );
  return { onShow, onNewSpace };
}

const card = (title: string) =>
  screen.getByRole("button", { name: `Show ${title}` }).closest("li") as HTMLElement;

test("a card per Space shows its color, icon, name, profile and pinned tabs, folders as one row", () => {
  renderCards();
  const cards = [...document.querySelectorAll<HTMLElement>(".space-card")];
  expect(cards.map((card) => card.querySelector(".space-card-name")?.textContent)).toEqual([
    "Home",
    "Work",
    "Play",
  ]);
  expect(cards[0].style.getPropertyValue("--card-color")).toBe("#3e7bfa");
  expect(cards[0].querySelector(".space-card-icon")?.textContent).toBe("🏠");
  expect(cards.map((card) => card.querySelector(".space-card-profile")?.textContent)).toEqual([
    "Default",
    "Office",
    "Default",
  ]);
  const work = screen.getByRole("list", { name: "Pinned in Work" });
  expect([...work.children].map((row) => row.textContent)).toEqual([
    "Mail",
    "Projects",
    "https://untitled.test/",
  ]);
  expect(work.querySelector("img")?.getAttribute("src")).toBe("https://ui.test/mail.png");
  // The cards hold no buttons for their pinned tabs: they only show them.
  expect(work.querySelector("button")).toBeNull();
});

test("only the Spaces matching the search show", () => {
  renderCards(vi.fn(), vi.fn(), "PROJ");
  expect([...document.querySelectorAll(".space-card-name")].map((c) => c.textContent)).toEqual([
    "Work",
  ]);
  expect(matches("", "anything")).toBe(true);
  expect(matches(" ma ", "Mail")).toBe(true);
  expect(matches("x", "Mail", "")).toBe(false);
});

test("clicking a card shows its Space, by its index in sidebar order", () => {
  const { onShow } = renderCards();
  fireEvent.click(card("Play").querySelector(".space-card-pinned") as Element);
  expect(onShow).toHaveBeenLastCalledWith(2);
  // The keyboard shows it from the name at once.
  fireEvent.click(screen.getByRole("button", { name: "Show Work" }), { detail: 0 });
  expect(onShow).toHaveBeenLastCalledWith(1);
});

test("a click on the name shows the Space once no second click follows; a double click renames it", () => {
  vi.useFakeTimers();
  const { onShow } = renderCards();
  const name = screen.getByRole("button", { name: "Show Home" });
  fireEvent.click(name, { detail: 1 });
  expect(onShow).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(RENAME_CLICK_MS));
  expect(onShow).toHaveBeenCalledWith(0);
  onShow.mockClear();
  fireEvent.click(name, { detail: 1 });
  fireEvent.click(name, { detail: 2 });
  fireEvent.doubleClick(name);
  act(() => vi.advanceTimersByTime(RENAME_CLICK_MS));
  expect(onShow).not.toHaveBeenCalled();
  const field = screen.getByRole("textbox", { name: "Space name" }) as HTMLInputElement;
  expect(document.activeElement).toBe(field);
  fireEvent.change(field, { target: { value: "Café/2" } });
  fireEvent.blur(field);
  expect(fetch).toHaveBeenLastCalledWith("/command/space/rename/13/Caf%C3%A9%2F2", {
    method: "POST",
  });
  expect(screen.queryByRole("textbox", { name: "Space name" })).toBeNull();
});

test("the profile pill, the options and the edit button act on their Space, not showing it", () => {
  const { onShow } = renderCards();
  fireEvent.click(card("Work").querySelector(".space-card-profile") as Element);
  expect(fetch).toHaveBeenLastCalledWith("/command/menu/space-profile/14", { method: "POST" });
  fireEvent.click(screen.getByRole("button", { name: "Work options" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/menu/space/14", { method: "POST" });
  const edit = screen.getByRole("button", { name: "Edit Work" });
  fireEvent.click(edit);
  expect(edit.getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "red" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/space/color/14/red", { method: "POST" });
  expect(screen.getByRole("textbox", { name: "Space emoji" })).toBeDefined();
  fireEvent.click(edit);
  expect(screen.queryByRole("textbox", { name: "Space emoji" })).toBeNull();
  expect(onShow).not.toHaveBeenCalled();
});

test("the + after the cards creates a Space", () => {
  const { onNewSpace } = renderCards();
  fireEvent.click(screen.getByRole("button", { name: "New Space" }));
  expect(onNewSpace).toHaveBeenCalledOnce();
});

test("a card dropped before or after another moves its Space there, as the app counts", () => {
  const order = [13, 14, 15];
  expect(spaceMoveCommand(order, 13, { id: 15, after: true })).toBe("space/move/13/2");
  expect(spaceMoveCommand(order, 13, { id: 15, after: false })).toBe("space/move/13/1");
  expect(spaceMoveCommand(order, 15, { id: 13, after: false })).toBe("space/move/15/0");
  expect(spaceMoveCommand(order, 14, { id: 14, after: false })).toBeNull();
  expect(spaceMoveCommand(order, 14, { id: 15, after: false })).toBeNull();
  expect(spaceMoveCommand(order, 14, { id: 13, after: true })).toBeNull();
});
