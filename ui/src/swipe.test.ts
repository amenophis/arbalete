import { expect, test } from "vitest";
import { type Gesture, SWIPE_DISTANCE, swipeTracker } from "./swipe";

// A swipe from touch to lift, moving the content by each of `deltas`.
function swipe(track: ReturnType<typeof swipeTracker>, deltas: number[]) {
  const gestures: Gesture[] = deltas.map((dx, i) => ({
    phase: i === 0 ? "begin" : "move",
    dx,
    time: i * 10,
  }));
  gestures.push({ phase: "end", dx: 0, time: deltas.length * 10 });
  return gestures.map(track);
}

test("switches to the next Space when a swipe moving the content left lifts past the distance", () => {
  const track = swipeTracker();
  const step = -SWIPE_DISTANCE / 4;
  expect(swipe(track, [step, step, step, step])).toEqual([null, null, null, null, "space/next"]);
});

test("switches to the previous Space when the content moves right", () => {
  const track = swipeTracker();
  expect(swipe(track, [SWIPE_DISTANCE]).at(-1)).toBe("space/previous");
});

test("a short swipe keeps the Space, and the next one starts from zero", () => {
  const track = swipeTracker();
  expect(swipe(track, [SWIPE_DISTANCE - 1]).at(-1)).toBeNull();
  expect(swipe(track, [SWIPE_DISTANCE - 1]).at(-1)).toBeNull();
});
