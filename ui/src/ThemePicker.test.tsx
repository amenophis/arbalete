import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { PALETTES, ThemePicker } from "./ThemePicker";
import type { Theme } from "./theme";

const fetch = vi.fn();

beforeEach(() => {
  vi.useFakeTimers();
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function show(theme: Partial<Theme> = {}) {
  render(<ThemePicker />);
  const detail = {
    space: 4,
    theme: { colors: [], mode: "auto", intensity: 1, noise: 0, ...theme },
  };
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:theme", { detail }));
  });
}

// The theme last sent for Space 4, once the picker has sent it.
function sent(): Theme {
  act(() => {
    vi.advanceTimersByTime(20);
  });
  const url = String(fetch.mock.lastCall?.[0]);
  const prefix = "/command/space/theme/4/";
  expect(url.startsWith(prefix)).toBe(true);
  return JSON.parse(decodeURIComponent(url.slice(prefix.length)));
}

const button = (name: string) => screen.getByRole("button", { name }) as HTMLButtonElement;
const slider = (name: string) => screen.getByRole("slider", { name });

test("without colors, invites to pick one and leaves the intensity and the grain off", () => {
  show();
  expect(screen.getByText("Tap to pick a color for this space")).toBeDefined();
  expect(button("Remove a color").disabled).toBe(true);
  expect(slider("Intensity").getAttribute("aria-disabled")).toBe("true");
  expect(slider("Grain").getAttribute("aria-disabled")).toBe("true");
  expect(button("Automatic appearance").getAttribute("aria-pressed")).toBe("true");
});

test("adds colors that go with the leading one, up to three, and removes the last", () => {
  show({ colors: ["#ff0000"] });
  fireEvent.click(button("Add a color"));
  expect(sent().colors).toHaveLength(2);
  fireEvent.click(button("Add a color"));
  expect(sent().colors).toHaveLength(3);
  expect(button("Add a color").disabled).toBe(true);
  expect(document.querySelectorAll(".theme-dot")).toHaveLength(3);
  fireEvent.click(button("Remove a color"));
  expect(sent().colors).toHaveLength(2);
});

test("a palette color leads, keeping the number of colors; its arrows page through more", () => {
  show({ colors: ["#ff0000", "#00ffff"] });
  fireEvent.click(button(PALETTES[0][3]));
  const theme = sent();
  expect(theme.colors).toHaveLength(2);
  expect(theme.colors[0]).toBe(PALETTES[0][3]);
  expect(button("Previous colors").disabled).toBe(true);
  fireEvent.click(button("More colors"));
  expect(button(PALETTES[1][0])).toBeDefined();
});

test("forces an appearance, and changes the intensity and the grain with the keyboard", () => {
  show({ colors: ["#6daee9"], intensity: 0.5, noise: 0.5 });
  fireEvent.click(button("Dark appearance"));
  expect(sent().mode).toBe("dark");
  fireEvent.keyDown(slider("Intensity"), { key: "ArrowRight" });
  expect(sent().intensity).toBeCloseTo(0.55);
  fireEvent.keyDown(slider("Grain"), { key: "ArrowLeft" });
  expect(sent().noise).toBeCloseTo(0.45);
});

test("closes on Escape and on losing the focus", () => {
  show();
  fireEvent.keyDown(window, { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/theme/hide", { method: "POST" });
  fetch.mockClear();
  fireEvent.blur(window);
  expect(fetch).toHaveBeenLastCalledWith("/command/theme/hide", { method: "POST" });
});
