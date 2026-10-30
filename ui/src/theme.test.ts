import { expect, test } from "vitest";
import {
  blend,
  colorAt,
  fromHsl,
  harmony,
  luminance,
  paint,
  pointOf,
  type Theme,
  toHsl,
  wheelFor,
} from "./theme";

const theme = (colors: string[], mode: Theme["mode"] = "auto"): Theme => ({
  colors,
  mode,
  intensity: 1,
  noise: 0.5,
});

test("converts colors to and from hue, saturation and lightness", () => {
  expect(toHsl("#ff0000")).toEqual({ h: 0, s: 1, l: 0.5 });
  expect(toHsl("#808080")).toEqual({ h: 0, s: 0, l: 128 / 255 });
  expect(fromHsl({ h: 120, s: 1, l: 0.5 })).toBe("#00ff00");
  expect(fromHsl(toHsl("#6daee9"))).toBe("#6daee9");
});

test("a dot's angle is its color's hue, clockwise from the right, and its distance its lightness", () => {
  const wheel = { saturation: 1, range: [0.2, 0.8] as [number, number] };
  // At the center, the range's lowest lightness; at the edge, its highest.
  expect(toHsl(colorAt(wheel, 0, 0)).l).toBeCloseTo(0.2, 2);
  expect(toHsl(colorAt(wheel, 0.99, 0)).l).toBeCloseTo(0.8, 2);
  expect(toHsl(colorAt(wheel, 5, 0)).l).toBeCloseTo(0.8, 2);
  // Downward on screen is a quarter turn clockwise.
  expect(toHsl(colorAt(wheel, 0, 0.5)).h).toBeCloseTo(90, 0);
  const color = colorAt(wheel, -0.3, -0.4);
  const point = pointOf(wheel, color);
  expect(point.x).toBeCloseTo(-0.3, 1);
  expect(point.y).toBeCloseTo(-0.4, 1);
});

test("the pad's range follows the leading color, the mode narrowing it to its half", () => {
  expect(wheelFor(theme([]))).toEqual({ saturation: 0.8, range: [0.01, 0.97] });
  const lead = "#6daee9";
  const { s, l } = toHsl(lead);
  const auto = wheelFor(theme([lead]));
  expect(auto.saturation).toBeCloseTo(s);
  expect(auto.range[0]).toBeCloseTo(l - 0.25);
  expect(auto.range[1]).toBeCloseTo(Math.min(0.97, l + 0.25));
  expect(wheelFor(theme([lead], "light")).range).toEqual([0.575, 0.97]);
  // A light color in dark mode widens the dark half up to it.
  expect(wheelFor(theme([lead], "dark")).range).toEqual([0.01, l]);
});

test("the colors that go with a leading one are its complement, or both sides of it", () => {
  const hue = (color: string) => Math.round(toHsl(color).h);
  expect(harmony("#ff0000", 1)).toEqual(["#ff0000"]);
  expect(harmony("#ff0000", 2).map(hue)).toEqual([0, 180]);
  expect(harmony("#ff0000", 3).map(hue)).toEqual([0, 150, 210]);
});

test("paints the colors over the neutrals of their appearance, at their intensity", () => {
  // Without colors, the system's neutrals, without grain.
  expect(paint(theme([]), true)).toEqual({ stops: ["#1b1820"], grain: 0, dark: true });
  expect(paint(theme([]), false).dark).toBe(false);
  // In auto mode, light colors are light, whatever the system's appearance.
  expect(paint(theme(["#f0e8d0"]), true)).toEqual({ stops: ["#f0e8d0"], grain: 0.5, dark: false });
  expect(paint(theme(["#202060"]), false).dark).toBe(true);
  // A forced mode keeps its neutrals; no intensity leaves only them.
  const faint = { ...theme(["#ff0000"], "light"), intensity: 0 };
  expect(paint(faint, true).stops).toEqual(["#eeebf3"]);
});

test("blends two paints stop by stop, the shorter one repeating its last", () => {
  const black = { stops: ["#000000"], grain: 0, dark: true };
  const white = { stops: ["#ffffff", "#ffffff"], grain: 1, dark: false };
  expect(blend(black, white, 0.25)).toEqual({
    stops: ["#404040", "#404040"],
    grain: 0.25,
    dark: true,
  });
  expect(blend(black, white, 0.75).dark).toBe(false);
  expect(luminance("#ffffff")).toBe(1);
});
