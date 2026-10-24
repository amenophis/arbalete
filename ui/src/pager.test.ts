import { expect, test } from "vitest";
import type { Space } from "./bridge";
import {
  bounds,
  FLICK_SPEED,
  FLICK_WINDOW_MS,
  follow,
  GAIN,
  OVERSHOOT,
  resist,
  spring,
  target,
  tint,
  velocity,
} from "./pager";

test("the pages follow the fingers, slower than they move", () => {
  expect(follow(1, -100, 200)).toBeCloseTo(1 + (100 * GAIN) / 200);
  expect(follow(1, 100, 200)).toBeCloseTo(1 - (100 * GAIN) / 200);
  expect(follow(1, 0, 200)).toBe(1);
});

test("a swipe reaches at most the pages next to where it began, within the Spaces", () => {
  expect(bounds(2, 5)).toEqual([1, 3]);
  expect(bounds(0, 5)).toEqual([0, 1]);
  expect(bounds(4, 5)).toEqual([3, 4]);
  expect(bounds(0, 1)).toEqual([0, 0]);
});

test("past its bounds, a position resists more and more, never past the overshoot", () => {
  expect(resist(0.5, [0, 1])).toBe(0.5);
  const near = resist(-0.1, [0, 1]);
  const far = resist(-1, [0, 1]);
  const farther = resist(-100, [0, 1]);
  expect(near).toBeLessThan(0);
  expect(near).toBeGreaterThan(-0.1);
  expect(far).toBeLessThan(near);
  expect(farther).toBeLessThan(far);
  expect(farther).toBeGreaterThan(-OVERSHOOT);
  expect(resist(1 + 100, [0, 1])).toBeLessThan(1 + OVERSHOOT);
  expect(resist(1.1, [0, 1]) - 1).toBeCloseTo(-resist(-0.1, [0, 1]));
});

test("the speed counts only the last moments before the release", () => {
  const samples = [
    { time: 0, position: 0 },
    { time: 500, position: 0.2 },
    { time: 550, position: 0.3 },
    { time: 600, position: 0.4 },
  ];
  expect(velocity(samples, 600)).toBeCloseTo(0.2 / 50);
  // Fingers that rest before lifting do not flick.
  expect(velocity(samples, 600 + FLICK_WINDOW_MS + 1)).toBe(0);
  expect(velocity([], 0)).toBe(0);
  expect(velocity([{ time: 0, position: 1 }], 0)).toBe(0);
});

test("a slow release lands on the nearest page", () => {
  const slow = FLICK_SPEED / 2;
  expect(target(1.6, slow, 1, 5)).toBe(2);
  expect(target(1.4, slow, 1, 5)).toBe(1);
  expect(target(0.6, -slow, 1, 5)).toBe(1);
  expect(target(0.4, -slow, 1, 5)).toBe(0);
});

test("a flick goes on to the next page in its direction", () => {
  const fast = FLICK_SPEED * 2;
  expect(target(1.1, fast, 1, 5)).toBe(2);
  expect(target(1, fast, 1, 5)).toBe(2);
  expect(target(0.9, -fast, 1, 5)).toBe(0);
  // Back towards where the swipe began.
  expect(target(1.8, -fast, 1, 5)).toBe(1);
});

test("a release lands at most one page from where the swipe began, within the Spaces", () => {
  const fast = FLICK_SPEED * 2;
  expect(target(2.2, fast, 1, 5)).toBe(2);
  expect(target(-0.2, -fast, 0, 5)).toBe(0);
  expect(target(4.2, fast, 4, 5)).toBe(4);
});

test("the spring settles on its target without going past it", () => {
  let crossed = false;
  for (let elapsed = 0; elapsed <= 500; elapsed += 16) {
    const { position } = spring(0.4, 1, 0, elapsed);
    if (position > 1) crossed = true;
  }
  expect(crossed).toBe(false);
  expect(spring(0.4, 1, 0, 0).position).toBeCloseTo(0.4);
  expect(spring(0.4, 1, 0, 100).position).toBeGreaterThan(0.4);
  expect(spring(0.4, 1, 0, 500)).toEqual({ position: 1, settled: true });
});

test("the spring starts at the release speed", () => {
  const still = spring(0.4, 1, 0, 16).position;
  const thrown = spring(0.4, 1, 0.005, 16).position;
  expect(thrown).toBeGreaterThan(still);
});

const spaces: Space[] = [
  { id: 1, title: "Home", emoji: null, color: "blue" },
  { id: 2, title: "Work", emoji: null, color: "red" },
];

test("the tint blends the colors of the two Spaces in view", () => {
  expect(tint(spaces, 0)).toBe("#3e7bfa");
  expect(tint(spaces, 1)).toBe("#e5484d");
  expect(tint(spaces, 0.25)).toBe("color-mix(in srgb, #3e7bfa, #e5484d 25%)");
  expect(tint(spaces, -0.2)).toBe("#3e7bfa");
  expect(tint(spaces, 1.2)).toBe("#e5484d");
  expect(tint([], 0)).toBeUndefined();
});
