import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { COLORS, mix, type Space, send, type Tint, WindowPainter } from "./bridge";

// The Spaces' pages sit side by side and the sidebar shows one at a time. A position counts in
// pages: 0 shows the first Space, 0.5 half of it and half of the second. One more page after the
// last Space, at the Spaces' count, creates a Space, and one before the first, at -1, holds the
// library. These pages are the sidebar's alone: while one shows, the app still displays the last
// Space shown, and the content view its tab.

// How far the pages move for each point the fingers move.
export const GAIN = 0.7;
// A release this fast, in pages per millisecond over the last FLICK_WINDOW_MS, is a flick.
export const FLICK_SPEED = 0.0015;
export const FLICK_WINDOW_MS = 100;
// The farthest the pages go past the first or last page, in pages, however far the fingers go.
export const OVERSHOOT = 0.25;
// How hard the pages resist going past an end, from 0 to 1, as they start to.
const RESISTANCE = 0.5;
// The spring's rate, per millisecond: it settles in about 0.35 s, without bouncing.
export const SPRING_RATE = 0.02;
// Closer and slower than this, the spring has settled.
const SETTLED = 0.001;
// How far the fingers pull past the last Space, in pages before resisting, to arm the "+" that
// opens the page creating a Space.
export const ARM_PULL = 0.15;
// How much wider than the sidebar the library is, in points (LIBRARY_WIDTH in app/src/shell.rs):
// the page's card moves right as much on the way to it.
export const LIBRARY_SHIFT = 180;
// The share of the way to the library over which the first Space's page fades out.
const LIBRARY_FADE = 0.5;

export type Sample = { time: number; position: number };

// Where the fingers have taken the pages, from a position and a distance in points: content moving
// left, a negative distance, brings the next Space.
export function follow(from: number, distance: number, width: number): number {
  return from - (distance * GAIN) / width;
}

// A swipe goes at most one page from where it began, and not past the library before the first
// Space or the last of the `count` Spaces: past it, the fingers pull out a "+" instead, which
// leads to the page after it, creating a Space.
export function bounds(start: number, count: number): [number, number] {
  return [Math.max(-1, start - 1), start === count ? count : Math.min(count - 1, start + 1)];
}

// Whether the fingers pulled far enough past the last Space to open the page creating one.
export function armed(pull: number): boolean {
  return pull >= ARM_PULL;
}

// How far towards the library a position is, from 0 on the first Space to 1 on the library.
export function libraryProgress(position: number): number {
  return Math.min(1, Math.max(0, -position));
}

// The first Space's page fades out on the way to the library, and back in on the way back.
export function libraryFade(position: number): number {
  return Math.max(0, 1 - libraryProgress(position) / LIBRARY_FADE);
}

// Past its bounds, a position resists more and more and never goes past OVERSHOOT.
export function resist(position: number, [min, max]: [number, number]): number {
  const band = (over: number) => OVERSHOOT * (1 - 1 / (1 + (RESISTANCE * over) / OVERSHOOT));
  if (position < min) return min - band(min - position);
  if (position > max) return max + band(position - max);
  return position;
}

// The speed, in pages per millisecond, over the last FLICK_WINDOW_MS before `now`.
export function velocity(samples: Sample[], now: number): number {
  const recent = samples.filter((sample) => sample.time >= now - FLICK_WINDOW_MS);
  const last = samples.at(-1);
  if (!last || recent.length === 0) return 0;
  const first = recent[0];
  const elapsed = now - first.time;
  return elapsed > 0 ? (last.position - first.position) / elapsed : 0;
}

// The page a release lands on: a flick goes on to the next page in its direction, a slower release
// to the nearest page.
export function target(position: number, speed: number, start: number, count: number): number {
  const [min, max] = bounds(start, count);
  let page = Math.round(position);
  if (speed > FLICK_SPEED) page = Math.floor(position) + 1;
  if (speed < -FLICK_SPEED) page = Math.ceil(position) - 1;
  return Math.min(max, Math.max(min, page));
}

// A critically damped spring from `from` to `to`, `elapsed` milliseconds after it started at
// `speed` pages per millisecond.
export function spring(from: number, to: number, speed: number, elapsed: number) {
  const offset = from - to;
  const decay = Math.exp(-SPRING_RATE * elapsed);
  const position = to + (offset + (speed + SPRING_RATE * offset) * elapsed) * decay;
  const velocity = (speed - SPRING_RATE * elapsed * (speed + SPRING_RATE * offset)) * decay;
  const settled = Math.abs(position - to) < SETTLED && Math.abs(velocity) < SETTLED / 10;
  return { position: settled ? to : position, settled };
}

// The tint between the two pages a position shows: a page that is no Space's has no color, and
// the other page's color fades towards none.
export function tint(spaces: Space[], position: number): Tint | undefined {
  if (spaces.length === 0) return undefined;
  const clamped = Math.min(spaces.length, Math.max(-1, position));
  const left = Math.floor(clamped);
  const share = clamped - left;
  const color = (page: number) => spaces[page] && COLORS[spaces[page].color];
  const [from, to] = [color(left), color(left + 1)];
  return {
    color: from && to ? mix(to, from, share * 100) : (from ?? to ?? COLORS.gray),
    strength: (from ? 1 - share : 0) + (to ? share : 0),
  };
}

// A trackpad swipe, from touch to lift, sent by the app as `arbalette:swipe` events (Gesture in
// app/src/swipe.rs): `dx` is how far the content would scroll, in points.
// `y` is where the pointer is, in points from the window's top.
export type Gesture = { phase: "begin" | "move" | "end"; dx: number; y: number; time: number };

// Moves the pages with the fingers, then lets a spring take them to a page, which the app then
// displays if it is a Space's. Pages move with a transform, frame by frame, without rendering. The
// displayed Space changing otherwise, with a shortcut or the switcher, slides its page in.
//
// The sidebar beside the page also moves what the app draws around it, once per frame at most:
// the window's colors follow the tint, and the page's card moves right on the way to the library,
// which the app widens once the pages rest on it.
class Pager {
  spaces: Space[] = [];
  displayed = 0;
  private displayedId?: number;
  sidebar: HTMLElement | null = null;
  track: HTMLElement | null = null;
  bubble: HTMLElement | null = null;
  // Told the page the pages come to rest on.
  onSettle: (page: number) => void = () => {};
  private readonly native = location.hash === "";
  private readonly painter = new WindowPainter();
  private card = "card/offset/0";
  // The page the pages are on, or going to.
  private page = 0;
  private position = 0;
  private started = false;
  private swipe: { start: number; from: number; distance: number; width: number } | null = null;
  private samples: Sample[] = [];
  // How far the fingers pull past the last Space, in pages, and where the pointer is.
  private pull = 0;
  private y = 0;
  private motion: { from: number; to: number; speed: number; start?: number } | null = null;
  private frame = 0;

  show(displayed: number) {
    // Before the app's first state, there is nothing to show.
    if (this.spaces.length === 0) return;
    // Spaces reordered from the library move the displayed one's page, without displaying
    // another Space: the pages stay where they rest.
    const id = this.spaces[displayed]?.id;
    const moved = id !== this.displayedId;
    if (!moved && displayed !== this.displayed && this.page === this.displayed) {
      this.page = displayed;
      this.position = displayed;
      this.onSettle(displayed);
    }
    this.displayed = displayed;
    this.displayedId = id;
    if (!this.started) {
      this.started = true;
      this.page = displayed;
      this.position = displayed;
      this.onSettle(displayed);
    } else if (moved && !this.swipe && this.motion?.to !== displayed) {
      this.slide(displayed);
    }
    this.render();
  }

  // Slides to a page: the app displays a Space first, and its page then slides in.
  go(page: number) {
    const space = this.spaces[page];
    if (space && page !== this.displayed) return send(`space/${space.id}`);
    this.slide(page);
  }

  // Only the pages next to the displayed one exist: a farther one starts from its neighbor.
  private slide(page: number) {
    const gap = page - this.position;
    if (Math.abs(gap) > 1) this.position = page - Math.sign(gap);
    this.settle(page, 0);
  }

  gesture({ phase, dx, y, time }: Gesture) {
    if (phase === "begin") {
      this.motion = null;
      const width = this.track?.clientWidth || 1;
      // The page shown and its neighbors are the ones that exist.
      this.swipe = { start: this.page, from: this.position, distance: 0, width };
      this.samples = [];
    }
    const swipe = this.swipe;
    if (!swipe) return;
    swipe.distance += dx;
    const raw = follow(swipe.from, swipe.distance, swipe.width);
    this.samples = [
      ...this.samples.filter((s) => s.time >= time - FLICK_WINDOW_MS),
      { time, position: raw },
    ];
    const count = this.spaces.length;
    const [min, max] = bounds(swipe.start, count);
    this.position = resist(raw, [min, max]);
    this.pull = swipe.start === count - 1 ? Math.max(0, raw - max) : 0;
    this.y = y;
    if (phase === "end") this.release(swipe.start, time);
    this.schedule();
  }

  private release(start: number, time: number) {
    this.swipe = null;
    const count = this.spaces.length;
    const speed = velocity(this.samples, time);
    const page = armed(this.pull) ? count : target(this.position, speed, start, count);
    this.pull = 0;
    const toward = Math.sign(speed) === Math.sign(page - this.position);
    this.settle(page, toward ? speed : 0);
    const space = this.spaces[page];
    if (page !== this.displayed && space) send(`space/${space.id}`);
  }

  private settle(to: number, speed: number) {
    this.page = to;
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      this.motion = null;
      this.position = to;
      this.onSettle(to);
    } else {
      this.motion = { from: this.position, to, speed };
    }
    this.schedule();
  }

  private schedule() {
    if (!this.frame) this.frame = requestAnimationFrame((now) => this.step(now));
  }

  private step(now: number) {
    this.frame = 0;
    const motion = this.motion;
    if (motion) {
      motion.start ??= now;
      const { position, settled } = spring(
        motion.from,
        motion.to,
        motion.speed,
        now - motion.start,
      );
      this.position = position;
      if (settled) {
        this.motion = null;
        this.onSettle(motion.to);
      } else this.schedule();
    }
    this.render();
  }

  render() {
    if (this.track) this.track.style.transform = `translateX(${-this.position * 100}%)`;
    const color = tint(this.spaces, this.position);
    if (this.sidebar && color) {
      this.sidebar.style.setProperty("--space", color.color);
      this.sidebar.style.setProperty("--tinted", String(color.strength));
    }
    const first = this.spaces[0];
    const firstPage = first && this.track?.querySelector<HTMLElement>(`[data-space="${first.id}"]`);
    if (firstPage) {
      const fade = libraryFade(this.position);
      firstPage.style.opacity = fade < 1 ? String(fade) : "";
    }
    if (this.bubble) {
      const reach = Math.min(1, this.pull / ARM_PULL);
      this.bubble.style.opacity = String(reach);
      this.bubble.style.transform = `translate(${(1 - reach) * 100}%, ${this.y}px)`;
      this.bubble.toggleAttribute("data-armed", armed(this.pull));
    }
    const space = this.spaces[this.displayed];
    if (this.native && color && space) {
      this.painter.paint(color, COLORS[space.color]);
      this.moveCard();
    }
  }

  // The card follows the pages towards the library, and rests beside it once they do.
  private moveCard() {
    const resting = !this.motion && !this.swipe && this.position === -1;
    const offset = Math.round(libraryProgress(this.position) * LIBRARY_SHIFT);
    const card = resting ? "library/show" : `card/offset/${offset}`;
    if (card === this.card) return;
    this.card = card;
    send(card);
  }

  start() {
    this.painter.start();
  }

  stop() {
    cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.painter.stop();
  }
}

// The sidebar, the track holding the pages and the "+" pulled past the last Space, to give to
// their elements, the page the pages rest on, and `go` to slide to a page, for the Spaces in
// sidebar order and the displayed one's index.
export function usePager(spaces: Space[], displayed: number) {
  const [pager] = useState(() => new Pager());
  const [settled, setSettled] = useState(displayed);
  pager.onSettle = setSettled;
  const sidebar = useRef<HTMLElement>(null);
  const track = useRef<HTMLDivElement>(null);
  const bubble = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    pager.sidebar = sidebar.current;
    pager.track = track.current;
    pager.bubble = bubble.current;
    pager.spaces = spaces;
    pager.show(displayed);
  });
  useEffect(() => {
    const listener = (event: Event) => pager.gesture((event as CustomEvent<Gesture>).detail);
    addEventListener("arbalette:swipe", listener);
    pager.start();
    return () => {
      removeEventListener("arbalette:swipe", listener);
      pager.stop();
    };
  }, [pager]);
  return { sidebar, track, bubble, settled, go: (page: number) => pager.go(page) };
}
