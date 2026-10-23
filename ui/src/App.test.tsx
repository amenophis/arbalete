import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { App } from "./App";

test("shows the app name", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Arbalette" })).toBeDefined();
});
