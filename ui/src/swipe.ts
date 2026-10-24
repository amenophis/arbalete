// A pause this long between wheel events ends a gesture, the trackpad's momentum included.
export const GESTURE_END_MS = 150;
// How far, in pixels, a horizontal swipe goes before it switches Space.
export const SWIPE_DISTANCE = 80;

export type SwipeCommand = "space/next" | "space/previous";

// A two-finger swipe on the trackpad reaches the page as wheel events. Mostly horizontal ones add
// up until they pass the distance, which switches Space once for the whole gesture: the Space
// after the displayed one when the fingers move left, as with pages, the one before when they
// move right. Mostly vertical ones are left to scroll the tabs.
export function swipeTracker() {
  let distance = 0;
  let switched = false;
  let last = Number.NEGATIVE_INFINITY;
  return (deltaX: number, deltaY: number, time: number): SwipeCommand | null => {
    if (time - last > GESTURE_END_MS) {
      distance = 0;
      switched = false;
    }
    last = time;
    if (switched || Math.abs(deltaX) <= Math.abs(deltaY)) return null;
    distance += deltaX;
    if (Math.abs(distance) < SWIPE_DISTANCE) return null;
    switched = true;
    return distance > 0 ? "space/next" : "space/previous";
  };
}
