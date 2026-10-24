import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { NewMenu } from "./NewMenu";

const fetch = vi.fn();
const onNewSpace = vi.fn();
const onProfiles = vi.fn();

beforeEach(() => {
  onNewSpace.mockReset();
  onProfiles.mockReset();
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
  render(<NewMenu onNewSpace={onNewSpace} onProfiles={onProfiles} />);
});

afterEach(cleanup);

const toggle = () => screen.getByRole("button", { name: "New" });
const menu = () => screen.queryByRole("menu");
const item = (name: string) => screen.getByRole("menuitem", { name });

test("the + button opens the menu on its first item, and closes it again", () => {
  expect(toggle().getAttribute("aria-expanded")).toBe("false");
  expect(menu()).toBeNull();
  fireEvent.click(toggle());
  expect(toggle().getAttribute("aria-expanded")).toBe("true");
  expect(screen.getAllByRole("menuitem").map((button) => button.textContent)).toEqual([
    "New Space",
    "New Folder⇧⌘N",
    "New Tab⌘T",
    "Profiles…",
  ]);
  expect(document.activeElement).toBe(item("New Space"));
  fireEvent.click(toggle());
  expect(menu()).toBeNull();
});

test("the items show a line icon, their shortcut, and separators between groups", () => {
  fireEvent.click(toggle());
  const children = [...(menu()?.children ?? [])].map((child) => child.tagName);
  expect(children).toEqual(["BUTTON", "BUTTON", "HR", "BUTTON", "HR", "BUTTON"]);
  for (const button of screen.getAllByRole("menuitem")) {
    expect(button.querySelector("svg[aria-hidden=true]")).not.toBeNull();
  }
  expect(item("New Tab").getAttribute("aria-keyshortcuts")).toBe("Meta+T");
  expect(item("New Tab").querySelector("kbd")?.textContent).toBe("⌘T");
  expect(item("New Space").querySelector("kbd")).toBeNull();
});

test("each item does its action and closes the menu", () => {
  const pick = (name: string) => {
    fireEvent.click(toggle());
    fireEvent.click(item(name));
    expect(menu()).toBeNull();
  };
  pick("New Tab");
  expect(fetch).toHaveBeenLastCalledWith("/command/overlay/show", { method: "POST" });
  pick("New Folder");
  expect(fetch).toHaveBeenLastCalledWith("/command/folder/new", { method: "POST" });
  pick("New Space");
  expect(onNewSpace).toHaveBeenCalledOnce();
  pick("Profiles…");
  expect(onProfiles).toHaveBeenCalledOnce();
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("the arrow keys move between the items, around the ends", () => {
  fireEvent.click(toggle());
  const focused = () => document.activeElement?.textContent;
  fireEvent.keyDown(document.activeElement as Element, { key: "ArrowUp" });
  expect(focused()).toBe("Profiles…");
  fireEvent.keyDown(document.activeElement as Element, { key: "ArrowDown" });
  expect(focused()).toBe("New Space");
  fireEvent.keyDown(document.activeElement as Element, { key: "ArrowDown" });
  expect(focused()).toBe("New Folder⇧⌘N");
  fireEvent.keyDown(document.activeElement as Element, { key: "ArrowDown" });
  expect(focused()).toBe("New Tab⌘T");
});

test("Escape closes the menu and gives the keyboard back to the + button", () => {
  fireEvent.click(toggle());
  fireEvent.keyDown(item("New Space"), { key: "Escape" });
  expect(menu()).toBeNull();
  expect(document.activeElement).toBe(toggle());
});

test("a press outside closes the menu, a press inside does not", () => {
  fireEvent.click(toggle());
  fireEvent.pointerDown(item("New Folder"));
  expect(menu()).not.toBeNull();
  fireEvent.pointerDown(document.body);
  expect(menu()).toBeNull();
  expect(fetch).not.toHaveBeenCalled();
});
