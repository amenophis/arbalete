import { mix } from "./bridge";

// How a Space colors the window (Theme in crates/arbalete-core/src/sidebar.rs): up to three
// colors, the first leading, shown over light or dark neutrals at an intensity, with grain.
export type Theme = {
  colors: string[];
  mode: Mode;
  intensity: number;
  noise: number;
};

// The system's appearance, or a forced one.
export type Mode = "auto" | "light" | "dark";

export const MAX_COLORS = 3;

// What the colors show over, and without colors.
const NEUTRALS = { light: "#eeebf3", dark: "#1b1820" };

// What a theme paints: the window's gradient, one to three colors, its grain from 0 to 1, and
// whether text over it is light.
export type Paint = { stops: string[]; grain: number; dark: boolean };

// The system's appearance is dark.
export function systemDark(): boolean {
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
}

// Whether a theme is dark: as its mode forces, or in auto mode, as its colors are, and as the system
// is without colors.
function isDark(theme: Theme, system: boolean): boolean {
  if (theme.mode !== "auto") return theme.mode === "dark";
  if (theme.colors.length === 0) return system;
  const average = theme.colors.reduce((sum, color) => sum + luminance(color), 0);
  return average / theme.colors.length < 0.4;
}

// A theme's paint: its colors over the neutrals of its appearance, at its intensity.
export function paint(theme: Theme, system: boolean): Paint {
  const dark = isDark(theme, system);
  const neutral = dark ? NEUTRALS.dark : NEUTRALS.light;
  if (theme.colors.length === 0) return { stops: [neutral], grain: 0, dark };
  const stops = theme.colors.map((color) => mix(color, neutral, theme.intensity * 100));
  return { stops, grain: theme.noise, dark };
}

// The paint of a page that is no Space's.
export function neutralPaint(system: boolean): Paint {
  return paint({ colors: [], mode: "auto", intensity: 0, noise: 0 }, system);
}

// Between two paints, `share` of the way to `to`: each stop blends with the one at its place, a
// paint with fewer stops repeating its last.
export function blend(from: Paint, to: Paint, share: number): Paint {
  const count = Math.max(from.stops.length, to.stops.length);
  const stop = (paint: Paint, i: number) => paint.stops[Math.min(i, paint.stops.length - 1)];
  return {
    stops: Array.from({ length: count }, (_, i) => mix(stop(to, i), stop(from, i), share * 100)),
    grain: from.grain + (to.grain - from.grain) * share,
    dark: share < 0.5 ? from.dark : to.dark,
  };
}

// The relative luminance of a #rrggbb color, from 0 to 1.
export function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

// A color's hue, from 0 to 360, saturation and lightness, from 0 to 1.
export type Hsl = { h: number; s: number; l: number };

export function toHsl(hex: string): Hsl {
  const [r, g, b] = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16) / 255);
  const [max, min] = [Math.max(r, g, b), Math.min(r, g, b)];
  const l = (max + min) / 2;
  if (max === min) return { h: 0, s: 0, l };
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h =
    max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return { h: h * 60, s, l };
}

export function fromHsl({ h, s, l }: Hsl): string {
  const a = s * Math.min(l, 1 - l);
  const channel = (n: number) => {
    const k = (n + h / 30) % 12;
    const value = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(value * 255)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${channel(0)}${channel(8)}${channel(4)}`;
}

// The color picker's pad, like Arc's: a dot's angle around the center is the color's hue, clockwise
// from the right, and its distance from the center its lightness, across `range`, at one saturation.
export type Wheel = { saturation: number; range: [number, number] };

// A dot at the pad's edge sits this far from the center, its radius being 1.
const EDGE = 0.99;

// The pad for a theme, from its leading color: as saturated, and its lightness ±0.25 in auto mode,
// or within the light or dark half the mode forces. Without colors, the whole range.
export function wheelFor(theme: Theme): Wheel {
  const [lowest, highest] = [0.01, 0.97];
  if (theme.colors.length === 0) return { saturation: 0.8, range: [lowest, highest] };
  const { s, l } = toHsl(theme.colors[0]);
  const saturation = Math.max(0.01, s);
  if (theme.mode === "auto") {
    return { saturation, range: [Math.max(lowest, l - 0.25), Math.min(highest, l + 0.25)] };
  }
  const [low, high] = theme.mode === "dark" ? [lowest, 0.425] : [0.575, highest];
  return { saturation, range: [Math.min(low, l), Math.max(high, l)] };
}

// The color at a point of the pad, from its center, its radius being 1.
export function colorAt(wheel: Wheel, x: number, y: number): string {
  const hue = ((Math.atan2(y, x) * 180) / Math.PI + 360) % 360;
  const t = Math.min(1, Math.max(0, Math.hypot(x, y) / EDGE));
  const [low, high] = wheel.range;
  return fromHsl({ h: hue, s: wheel.saturation, l: low + t * (high - low) });
}

// Where a color sits on the pad, from its center, its radius being 1.
export function pointOf(wheel: Wheel, color: string): { x: number; y: number } {
  const { h, l } = toHsl(color);
  const [low, high] = wheel.range;
  const t = high > low ? Math.min(1, Math.max(0, (l - low) / (high - low))) : 0;
  const angle = (h * Math.PI) / 180;
  return { x: Math.cos(angle) * t * EDGE, y: Math.sin(angle) * t * EDGE };
}

// The colors that go with a leading one, as many as the theme holds: its complement for two, both
// sides of it for three, as far from the center.
export function harmony(color: string, count: number): string[] {
  const hsl = toHsl(color);
  const offsets = [[], [0], [0, 180], [0, 150, 210]][count] ?? [];
  return offsets.map((offset) => (offset ? fromHsl({ ...hsl, h: (hsl.h + offset) % 360 }) : color));
}
