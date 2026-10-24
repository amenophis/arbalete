import { expect, test } from "vitest";
import { dominantColor } from "./faviconColor";

const pixels = (...colors: number[][]) => colors.flat();

test("finds the most common color of an icon, leaving out transparent and gray pixels", () => {
  const red = [230, 40, 40, 255];
  const blue = [20, 90, 220, 255];
  const icon = pixels(red, red, blue, [255, 255, 255, 255], [0, 0, 0, 255], [0, 200, 0, 0]);
  expect(dominantColor(icon)).toBe("#e62828");
  expect(dominantColor(pixels(red, blue, blue))).toBe("#145adc");
});

test("averages close shades of the dominant color", () => {
  expect(dominantColor(pixels([200, 40, 40, 255], [210, 50, 60, 255]))).toBe("#cd2d32");
});

test("finds no color in a gray, black or white icon", () => {
  expect(
    dominantColor(pixels([255, 255, 255, 255], [10, 10, 10, 255], [128, 130, 128, 255])),
  ).toBeNull();
  expect(dominantColor([])).toBeNull();
});
