import { beforeEach, expect, test, vi } from "vitest";
import { revealOnHover } from "./bridge";

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
