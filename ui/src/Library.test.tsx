import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Library } from "./Library";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

test("lists the tabs closed since launch, the last closed first, and reopens one by rank", () => {
  const onReopen = vi.fn();
  const closed = [
    { url: "https://example.com/last", title: "Last", favicon: null },
    { url: "https://example.com/untitled", title: "", favicon: null },
  ];
  render(<Library closed={closed} shown onReopen={onReopen} />);
  const rows = screen.getAllByRole("button");
  expect(rows.map((row) => row.textContent)).toEqual(["Last", "https://example.com/untitled"]);
  fireEvent.click(rows[1]);
  expect(fetch).toHaveBeenLastCalledWith("/command/reopen/1", { method: "POST" });
  expect(onReopen).toHaveBeenCalledOnce();
});

test("says when no tab has been closed", () => {
  render(<Library closed={[]} shown onReopen={() => {}} />);
  expect(screen.getByText("No tab closed yet")).toBeDefined();
});
