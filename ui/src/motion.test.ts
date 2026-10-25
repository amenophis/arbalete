import { afterEach, expect, test, vi } from "vitest";
import { FOLDER, INSERT, type Layout, MOVE, motionOf, REMOVE, SEPARATE, slideFrom } from "./motion";

const layout = (ids: string[], expanded: string[] = [], splits: string[] = []): Layout => ({
  ids,
  expanded,
  splits,
});

test("a folder opening or closing slides the rows below and fades its tabs", () => {
  const open = layout(["1", "2", "3", "4"], ["1"]);
  const closed = layout(["1", "4"], []);
  expect(motionOf(open, closed)).toEqual({ slide: FOLDER, fade: FOLDER });
  expect(motionOf(closed, open)).toEqual({ slide: FOLDER, fade: FOLDER });
});

test("a row inserted pushes the rows below and fades in, a row removed closes up", () => {
  expect(motionOf(layout(["1", "2"]), layout(["1", "9", "2"]))).toEqual({
    slide: INSERT,
    fade: INSERT,
  });
  expect(motionOf(layout(["1", "2", "3"]), layout(["1", "3"]))).toEqual({
    slide: REMOVE,
    fade: 0,
  });
});

test("a split separated crossfades into its tabs' rows", () => {
  const split = layout(["1", "5", "2"], [], ["5"]);
  expect(motionOf(split, layout(["1", "3", "4", "2"]))).toEqual({
    slide: INSERT,
    fade: SEPARATE,
  });
});

test("rows moved around slide, and nothing moves when the rows stay", () => {
  expect(motionOf(layout(["1", "2", "3"]), layout(["2", "1", "3"]))).toEqual({
    slide: MOVE,
    fade: 0,
  });
  expect(motionOf(layout(["1", "2"], ["1"]), layout(["1", "2"], ["1"]))).toBeNull();
});

afterEach(() => {
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

// jsdom lays nothing out: each row says where it is.
function row(id: string, top: number): HTMLElement {
  const item = document.createElement("li");
  item.dataset.id = id;
  item.getBoundingClientRect = () => new DOMRect(0, top, 200, 41);
  document.body.append(item);
  return item;
}

test("slides a moved row from its old place with its transform alone", () => {
  const [moved, still] = [row("1", 82), row("2", 0)];
  slideFrom(
    new Map([
      ["1", new DOMRect(0, 41, 200, 41)],
      ["2", new DOMRect(0, 0, 200, 41)],
    ]),
    document,
    "125ms ease-in-out",
  );
  expect(moved.style.transform).toBe("");
  expect(moved.style.transition).toBe("transform 125ms ease-in-out");
  expect(still.style.transition).toBe("");
});

test("does not move when motion is reduced", () => {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: query.includes("reduce") }));
  const moved = row("1", 82);
  slideFrom(new Map([["1", new DOMRect(0, 41, 200, 41)]]), document, "125ms ease-in-out");
  expect(moved.style.transition).toBe("");
  vi.unstubAllGlobals();
});
