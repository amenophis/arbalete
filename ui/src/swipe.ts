// How far, in points, a swipe goes before it switches Space.
export const SWIPE_DISTANCE = 80;

export type SwipeCommand = "space/next" | "space/previous";

// A horizontal swipe on the trackpad, from touch to lift, sent by the app as `arbalette:swipe`
// events (Gesture in app/src/swipe.rs): `dx` is how far the content would scroll, in points.
export type Gesture = { phase: "begin" | "move" | "end"; dx: number; time: number };

// A swipe that goes far enough switches Space once the fingers lift: the Space after the displayed
// one when the content moves left, as with pages, the one before when it moves right.
export function swipeTracker() {
  let distance = 0;
  return ({ phase, dx }: Gesture): SwipeCommand | null => {
    if (phase === "begin") distance = 0;
    distance += dx;
    if (phase !== "end" || Math.abs(distance) < SWIPE_DISTANCE) return null;
    return distance < 0 ? "space/next" : "space/previous";
  };
}
