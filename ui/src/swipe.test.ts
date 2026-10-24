import { expect, test } from "vitest";
import { GESTURE_END_MS, SWIPE_DISTANCE, swipeTracker } from "./swipe";

// Wheel events 10 ms apart, all in one gesture.
function gesture(track: ReturnType<typeof swipeTracker>, deltas: [number, number][], start = 0) {
  return deltas.map(([x, y], i) => track(x, y, start + i * 10));
}

test("switches to the next Space once a leftward swipe passes the distance", () => {
  const track = swipeTracker();
  const step = SWIPE_DISTANCE / 4;
  expect(
    gesture(track, [
      [step, 0],
      [step, 0],
      [step, 0],
      [step, 0],
    ]),
  ).toEqual([null, null, null, "space/next"]);
});

test("switches to the previous Space on a rightward swipe", () => {
  const track = swipeTracker();
  expect(track(-SWIPE_DISTANCE, 0, 0)).toBe("space/previous");
});

test("switches only once per gesture, momentum included", () => {
  const track = swipeTracker();
  const results = gesture(track, Array(20).fill([SWIPE_DISTANCE, 0]));
  expect(results.filter(Boolean)).toEqual(["space/next"]);
});

test("a pause ends the gesture, so the next one switches again", () => {
  const track = swipeTracker();
  expect(track(SWIPE_DISTANCE, 0, 0)).toBe("space/next");
  expect(track(SWIPE_DISTANCE, 0, GESTURE_END_MS)).toBeNull();
  expect(track(-SWIPE_DISTANCE, 0, 2 * GESTURE_END_MS + 1)).toBe("space/previous");
});

test("a pause also forgets a short swipe", () => {
  const track = swipeTracker();
  expect(track(SWIPE_DISTANCE - 1, 0, 0)).toBeNull();
  expect(track(SWIPE_DISTANCE - 1, 0, GESTURE_END_MS + 1)).toBeNull();
});

test("leaves mostly vertical scrolling alone", () => {
  const track = swipeTracker();
  const results = gesture(track, Array(20).fill([SWIPE_DISTANCE, SWIPE_DISTANCE + 1]));
  expect(results.filter(Boolean)).toEqual([]);
});
