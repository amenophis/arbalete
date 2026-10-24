import { expect, test } from "vitest";
import { END, moveCommand, rowPlace } from "./drag";

test("drops before a row on its upper half and after it on its lower half", () => {
  expect(rowPlace(2, 3, 0.2)).toEqual({ parent: 2, index: 3 });
  expect(rowPlace(2, 3, 0.7)).toEqual({ parent: 2, index: 4 });
});

test("drops before a folder on its top edge and into it below", () => {
  expect(rowPlace(2, 3, 0.1, 6)).toEqual({ parent: 2, index: 3 });
  expect(rowPlace(2, 3, 0.5, 6)).toEqual({ parent: 6, index: END });
  expect(rowPlace(2, 3, 0.9, 6)).toEqual({ parent: 6, index: END });
});

test("counts the index once the node has left its parent", () => {
  expect(moveCommand(5, { parent: 2, index: 1 }, { parent: 2, index: 4 })).toBe("move/5/2/3");
  expect(moveCommand(5, { parent: 2, index: 4 }, { parent: 2, index: 1 })).toBe("move/5/2/1");
  expect(moveCommand(5, { parent: 2, index: 1 }, { parent: 6, index: 0 })).toBe("move/5/6/0");
});

test("sends nothing when the node would stay in place", () => {
  expect(moveCommand(5, { parent: 2, index: 1 }, { parent: 2, index: 1 })).toBeNull();
  expect(moveCommand(5, { parent: 2, index: 1 }, { parent: 2, index: 2 })).toBeNull();
});
