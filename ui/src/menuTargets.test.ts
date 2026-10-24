import { act } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { menuTargets, trackMenuTargets } from "./menuTargets";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
  document.body.innerHTML = `
    <section class="space-page" data-space="13" inert><ul class="favorites"><li data-id="1"></li></ul></section>
    <section class="space-page" data-space="14">
      <ul class="favorites"><li data-id="10" data-rect="7,50,120,47"></li></ul>
      <div class="scroll" data-rect="0,100,260,400">
        <h2 class="space-header" data-rect="7,104,250,32"></h2>
        <ul class="tabs">
          <li class="folder-item" data-id="6" data-rect="7,140,250,123">
            <button class="folder" data-rect="7,140,250,41"></button>
            <ul><li class="tab-row" data-id="7" data-rect="19,181,238,41"></li></ul>
          </li>
          <li class="tab-row" data-id="3" data-rect="7,480,250,41"></li>
          <li class="tab-row" data-id="4" data-rect="7,600,250,41"></li>
        </ul>
      </div>
    </section>`;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const [x, y, width, height] = (this.getAttribute("data-rect") ?? "0,0,0,0")
      .split(",")
      .map(Number);
    return new DOMRect(x, y, width, height);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const viewport = new DOMRect(0, 0, 260, 800);

test("lists the displayed page's tiles, header, folder and tab rows, as much as shows of them", () => {
  expect(menuTargets(document, viewport)).toEqual([
    { target: { node: 10 }, x: 7, y: 50, width: 120, height: 47 },
    { target: { space: 14 }, x: 7, y: 104, width: 250, height: 32 },
    { target: { node: 6 }, x: 7, y: 140, width: 250, height: 41 },
    { target: { node: 7 }, x: 19, y: 181, width: 238, height: 41 },
    { target: { node: 3 }, x: 7, y: 480, width: 250, height: 20 },
  ]);
});

test("lists nothing while no Space's page shows", () => {
  document.querySelector('[data-space="14"]')?.setAttribute("inert", "");
  expect(menuTargets(document, viewport)).toEqual([]);
});

test("sends the targets when they change, once a frame at most", async () => {
  const frame = () => act(() => new Promise(requestAnimationFrame));
  trackMenuTargets(document.body, "revealed");
  await frame();
  expect(fetch).toHaveBeenCalledOnce();
  const [url] = fetch.mock.calls[0];
  const [prefix, json] = String(url).split(/(?<=revealed\/)/);
  expect(prefix).toBe("/command/menu/targets/revealed/");
  expect(JSON.parse(decodeURIComponent(json))).toEqual(menuTargets(document, viewport));
  document.body.setAttribute("data-unrelated", "1");
  await frame();
  expect(fetch).toHaveBeenCalledOnce();
  document.querySelector('[data-id="3"]')?.remove();
  document.querySelector('[data-id="4"]')?.remove();
  await frame();
  expect(fetch).toHaveBeenCalledTimes(2);
});
