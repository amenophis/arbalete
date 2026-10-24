import { beforeEach, expect, test, vi } from "vitest";
import { followDocking, mix, revealOnHover, windowColors } from "./bridge";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

test("reveals the sidebar while the pointer is over it", () => {
  const element = document.createElement("div");
  revealOnHover(element);
  element.dispatchEvent(new MouseEvent("mouseenter"));
  expect(fetch).toHaveBeenLastCalledWith("/command/sidebar/reveal", { method: "POST" });
  element.dispatchEvent(new MouseEvent("mouseleave", { clientX: 300 }));
  expect(fetch).toHaveBeenLastCalledWith("/command/sidebar/conceal", { method: "POST" });
});

test("keeps the revealed sidebar while the pointer leaves it towards the window's edge", () => {
  const element = document.createElement("div");
  revealOnHover(element);
  element.dispatchEvent(new MouseEvent("mouseleave", { clientX: -2 }));
  expect(fetch).not.toHaveBeenCalled();
});

test("slides the sidebar's content out as it hides, and back at once as it docks", () => {
  const root = document.createElement("div");
  followDocking(root);
  const docked = (docked: boolean, slide: boolean) =>
    dispatchEvent(new CustomEvent("arbalette:docked", { detail: { docked, slide } }));
  docked(false, true);
  expect(root.hasAttribute("data-undocked")).toBe(true);
  expect(root.hasAttribute("data-instant")).toBe(false);
  docked(true, false);
  expect(root.hasAttribute("data-undocked")).toBe(false);
  expect(root.hasAttribute("data-instant")).toBe(true);
});

test("mixes a share of a color into another, like color-mix in srgb", () => {
  expect(mix("#ff0000", "#0000ff", 0)).toBe("#0000ff");
  expect(mix("#ff0000", "#0000ff", 100)).toBe("#ff0000");
  expect(mix("#ffffff", "#000000", 50)).toBe("#808080");
  expect(mix("#3e7bfa", "#1b0e1f", 14)).toBe("#201d3e");
});

test("tints the window with a tint and an empty Space's card with the Space's color", () => {
  const black = { color: "#000000", strength: 1 };
  expect(windowColors(black, "#000000", "#ffffff", "#ffffff")).toBe("window/colors/dbdbdb/ebebeb");
  // A page that is no Space's leaves the window untinted, halfway to it half tinted.
  const none = { color: "#000000", strength: 0 };
  expect(windowColors(none, "#000000", "#ffffff", "#ffffff")).toBe("window/colors/ffffff/ebebeb");
  const half = { color: "#000000", strength: 0.5 };
  expect(windowColors(half, "#000000", "#ffffff", "#ffffff")).toBe("window/colors/ededed/ebebeb");
});

test("forgets the hover kept from before the sidebar hid or moved, until the pointer moves", () => {
  const root = document.createElement("div");
  followDocking(root);
  dispatchEvent(new CustomEvent("arbalette:docked", { detail: { docked: false, slide: true } }));
  expect(root.hasAttribute("data-stale-hover")).toBe(true);
  dispatchEvent(new PointerEvent("pointermove"));
  expect(root.hasAttribute("data-stale-hover")).toBe(false);
  const revealed = document.createElement("div");
  revealOnHover(revealed);
  revealed.dispatchEvent(new MouseEvent("mouseleave", { clientX: 300 }));
  expect(revealed.hasAttribute("data-stale-hover")).toBe(true);
  dispatchEvent(new PointerEvent("pointerdown"));
  expect(revealed.hasAttribute("data-stale-hover")).toBe(false);
});
