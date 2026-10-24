import { beforeEach, expect, test, vi } from "vitest";
import { mix, revealOnHover, windowColors } from "./bridge";

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
  element.dispatchEvent(new MouseEvent("mouseleave"));
  expect(fetch).toHaveBeenLastCalledWith("/command/sidebar/conceal", { method: "POST" });
});

test("mixes a share of a color into another, like color-mix in srgb", () => {
  expect(mix("#ff0000", "#0000ff", 0)).toBe("#0000ff");
  expect(mix("#ff0000", "#0000ff", 100)).toBe("#ff0000");
  expect(mix("#ffffff", "#000000", 50)).toBe("#808080");
  expect(mix("#3e7bfa", "#1b0e1f", 14)).toBe("#201d3e");
});

test("tints the window and an empty Space's card with the Space's color", () => {
  expect(windowColors("#000000", "#ffffff", "#ffffff")).toBe("window/colors/dbdbdb/ebebeb");
});
