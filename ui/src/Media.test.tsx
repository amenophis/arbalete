import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Download } from "./Downloads";
import { MediaList } from "./Media";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

function download(id: number, name: string, change: Partial<Download> = {}): Download {
  const base = { received: 1, total: 1, speed: 0, startedAt: 0, missing: false };
  return { id, name, url: `https://${name}.test/`, state: "complete", ...base, ...change };
}

test("previews the images and videos still there, the most recent first, and opens one", () => {
  render(
    <MediaList
      query=""
      downloads={[
        download(1, "photo.JPG"),
        download(2, "report.pdf"),
        download(3, "clip.webm"),
        download(4, "gone.png", { missing: true }),
        download(5, "half.png", { state: "inProgress" }),
      ]}
    />,
  );
  const tiles = screen.getAllByRole("button");
  expect(tiles.map((tile) => tile.textContent)).toEqual(["clip.webm", "photo.JPG"]);
  expect(tiles[0].querySelector("video")?.getAttribute("src")).toBe("/media/3");
  expect(tiles[1].querySelector("img")?.getAttribute("src")).toBe("/media/1");
  fireEvent.click(tiles[1]);
  expect(fetch).toHaveBeenLastCalledWith("/command/download/open/1", { method: "POST" });
});

test("searches the media by name, and says when there are none", () => {
  const { rerender } = render(<MediaList query="clip" downloads={[download(1, "photo.png")]} />);
  expect(screen.getByText("No media matches")).toBeDefined();
  rerender(<MediaList query="" downloads={[download(2, "notes.txt")]} />);
  expect(screen.getByText("No media yet")).toBeDefined();
});
