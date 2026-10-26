import { expect, test } from "vitest";
import { matches } from "./search";

test("a search matches any text holding it, whatever the case and the spaces around it", () => {
  expect(matches("", "anything")).toBe(true);
  expect(matches(" ma ", "Mail")).toBe(true);
  expect(matches("x", "Mail", "")).toBe(false);
});
