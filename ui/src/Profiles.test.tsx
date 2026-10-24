import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Profile } from "./bridge";
import { Profiles } from "./Profiles";

const fetch = vi.fn();
const onOpen = vi.fn();

beforeEach(() => {
  onOpen.mockReset();
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const profiles: Profile[] = [
  { id: 1, name: "Default", color: "purple" },
  { id: 7, name: "Work", color: "blue" },
];

function open() {
  render(<Profiles profiles={profiles} open onOpen={onOpen} />);
}

const names = () => screen.getAllByRole("textbox", { name: "Profile name" }) as HTMLInputElement[];

test("lists the profiles only while open", () => {
  const { rerender } = render(<Profiles profiles={profiles} open={false} onOpen={onOpen} />);
  expect(screen.queryByRole("list", { name: "Profiles" })).toBeNull();
  rerender(<Profiles profiles={profiles} open onOpen={onOpen} />);
  expect(names().map((input) => input.value)).toEqual(["Default", "Work"]);
});

test("closes on Escape or a press outside, not on a press inside", () => {
  open();
  fireEvent.pointerDown(screen.getByRole("button", { name: "+ New profile" }));
  expect(onOpen).not.toHaveBeenCalled();
  fireEvent.pointerDown(document.body);
  expect(onOpen).toHaveBeenLastCalledWith(false);
  onOpen.mockReset();
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(onOpen).toHaveBeenLastCalledWith(false);
});

test("creates a profile", () => {
  open();
  fireEvent.click(screen.getByRole("button", { name: "+ New profile" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/profile/new", { method: "POST" });
});

test("renames a profile with Enter", () => {
  open();
  const input = names()[1];
  input.focus();
  fireEvent.change(input, { target: { value: " Café/2 " } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(fetch).toHaveBeenLastCalledWith("/command/profile/rename/7/Caf%C3%A9%2F2", {
    method: "POST",
  });
});

test("keeps the name on Escape or when it is blank", () => {
  open();
  const input = names()[0];
  fireEvent.change(input, { target: { value: "Other" } });
  fireEvent.keyDown(input, { key: "Escape" });
  fireEvent.change(input, { target: { value: "  " } });
  fireEvent.blur(input);
  expect(input.value).toBe("Default");
  expect(fetch).not.toHaveBeenCalled();
});

test("shows the color of each profile and picks another", () => {
  open();
  const work = screen.getByRole("group", { name: "Color of Work" });
  expect(within(work).getAllByRole("button")).toHaveLength(8);
  expect(within(work).getByRole("button", { name: "blue" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  expect(within(work).getByRole("button", { name: "green" }).getAttribute("aria-pressed")).toBe(
    "false",
  );
  fireEvent.click(within(work).getByRole("button", { name: "green" }));
  expect(fetch).toHaveBeenLastCalledWith("/command/profile/color/7/green", { method: "POST" });
});

test("edits the name of the profile the app has just created, then stays open", () => {
  render(<Profiles profiles={profiles} open={false} onOpen={onOpen} />);
  act(() => {
    dispatchEvent(new CustomEvent("arbalette:rename-profile", { detail: 7 }));
  });
  const input = names()[1];
  expect(document.activeElement).toBe(input);
  fireEvent.blur(input);
  expect(onOpen).toHaveBeenLastCalledWith(true);
});

test("opens the list to edit the name of a profile the app has just created", () => {
  render(<Profiles profiles={profiles} open={false} onOpen={onOpen} />);
  act(() => {
    dispatchEvent(new CustomEvent("arbalette:rename-profile", { detail: 1 }));
  });
  expect(document.activeElement).toBe(names()[0]);
});
