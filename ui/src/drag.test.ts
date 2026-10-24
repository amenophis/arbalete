import { expect, test } from "vitest";
import { END, moveCommand, previewTiles, rowPlace, tileSlot, zoneAt } from "./drag";

test("a tab on the middle of another tab row makes a split, its edges insert", () => {
  expect(zoneAt(0.1, "tab", "tab")).toBe("before");
  expect(zoneAt(0.3, "tab", "tab")).toBe("split");
  expect(zoneAt(0.7, "tab", "tile")).toBe("split");
  expect(zoneAt(0.8, "tab", "tab")).toBe("after");
});

test("folders and splits never make a split, nor does a tab on a split row", () => {
  expect(zoneAt(0.3, "tab", "folder")).toBe("before");
  expect(zoneAt(0.6, "tab", "split")).toBe("after");
  expect(zoneAt(0.4, "split", "tab")).toBe("before");
  expect(zoneAt(0.6, "split", "tab")).toBe("after");
});

test("drops before a folder on its upper half and first into it below", () => {
  expect(zoneAt(0.4, "folder", "tab")).toBe("before");
  expect(zoneAt(0.6, "folder", "folder")).toBe("into");
  expect(rowPlace(2, 3, "before", 6)).toEqual({ parent: 2, index: 3 });
  expect(rowPlace(2, 3, "into", 6)).toEqual({ parent: 6, index: 0 });
});

test("drops before or after a row", () => {
  expect(rowPlace(2, 3, "before")).toEqual({ parent: 2, index: 3 });
  expect(rowPlace(2, 3, "after")).toEqual({ parent: 2, index: 4 });
  expect(END).toBe(Number.MAX_SAFE_INTEGER);
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

const grid = { left: 10, top: 100, width: 240 };

test("finds the favorites' cell under the pointer, by rows of balanced columns", () => {
  // 6 tiles: two rows of 3, 80 px wide cells, 55 px apart.
  expect(tileSlot(15, 110, grid, 6)).toBe(0);
  expect(tileSlot(100, 110, grid, 6)).toBe(1);
  expect(tileSlot(245, 170, grid, 6)).toBe(5);
  // A row dropped as the 7th tile: two rows of 4.
  expect(tileSlot(245, 110, grid, 7)).toBe(3);
  expect(tileSlot(245, 170, grid, 7)).toBe(6);
});

test("keeps the cell within the grid when the pointer is past its edges", () => {
  expect(tileSlot(0, 90, grid, 3)).toBe(0);
  expect(tileSlot(400, 400, grid, 3)).toBe(2);
  expect(tileSlot(400, 90, grid, 1)).toBe(0);
});

const tiles = [1, 2, 3, 4].map((id) => ({ id }));

test("moves the dragged tile to its slot, the others keeping their order", () => {
  const ids = (shown: ({ id: number } | null)[]) => shown.map((tile) => tile?.id ?? null);
  expect(ids(previewTiles(tiles, 9, { parent: 9, slot: 2, tile: 1 }))).toEqual([2, 3, 1, 4]);
  expect(ids(previewTiles(tiles, 9, { parent: 9, slot: 0, tile: 4 }))).toEqual([4, 1, 2, 3]);
  expect(ids(previewTiles(tiles, 9, { parent: 9, slot: 1, tile: null }))).toEqual([
    1,
    null,
    2,
    3,
    4,
  ]);
});

test("shows the favorites as they are without a preview of theirs", () => {
  expect(previewTiles(tiles, 9, null)).toBe(tiles);
  expect(previewTiles(tiles, 9, { parent: 8, slot: 1, tile: null })).toBe(tiles);
  expect(previewTiles(tiles, 9, { parent: 9, slot: 1, tile: 7 })).toBe(tiles);
});
