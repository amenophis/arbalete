import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { Overlay, RESULT_ROW, SEARCH_ROW, toUrl } from "./Overlay";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

const field = () => screen.getByRole("textbox", { name: "Search or enter URL" });
const commands = () => fetch.mock.calls.map(([url]) => url as string);
const rows = () => screen.queryAllByRole("button").map((row) => row.textContent);
const picked = () => document.querySelector("[aria-current]")?.textContent;

function show(url: string | null = null, favicon: string | null = null) {
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:show", { detail: { url, favicon } }));
  });
}

function answer(text: string, tabs: object[], pages: object[]) {
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:results", { detail: { text, tabs, pages } }));
  });
}

test("opens the typed address in a new tab", () => {
  render(<Overlay />);
  show();
  fireEvent.change(field(), { target: { value: "example.com/été" } });
  expect(rows()).toEqual(["example.com/été"]);
  fireEvent.submit(field());
  expect(fetch).toHaveBeenLastCalledWith(
    "/command/open/https%3A%2F%2Fexample.com%2F%C3%A9t%C3%A9",
    { method: "POST" },
  );
});

test("searches what is not an address", () => {
  render(<Overlay />);
  show();
  fireEvent.change(field(), { target: { value: "arc browser" } });
  expect(commands()).toContain("/command/overlay/search/arc%20browser");
  expect(rows()).toEqual(["arc browser — Search Google"]);
  fireEvent.submit(field());
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/open/${encodeURIComponent("https://www.google.com/search?q=arc%20browser")}`,
    { method: "POST" },
  );
});

test("lists the matching open tabs then history pages, picked with the arrows, and fits them", () => {
  render(<Overlay />);
  show();
  fireEvent.change(field(), { target: { value: "pedia" } });
  answer(
    "pedia",
    [{ id: 7, url: "https://en.wikipedia.org/", title: "Wikipedia", favicon: null }],
    [{ url: "https://wiki.test/", title: "" }],
  );
  expect(rows()).toEqual([
    "pedia — Search Google",
    "WikipediaSwitch to Tab →",
    "https://wiki.test/",
  ]);
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/overlay/height/${SEARCH_ROW + 3 * RESULT_ROW + 16}`,
    { method: "POST" },
  );
  expect(picked()).toBe("pedia — Search Google");
  fireEvent.keyDown(field(), { key: "ArrowDown" });
  expect(picked()).toBe("WikipediaSwitch to Tab →");
  fireEvent.submit(field());
  expect(commands().slice(-2)).toEqual(["/command/select/7", "/command/overlay/hide"]);
  // The arrows wrap around.
  fireEvent.keyDown(field(), { key: "ArrowUp" });
  fireEvent.keyDown(field(), { key: "ArrowUp" });
  expect(picked()).toBe("https://wiki.test/");
  fireEvent.click(screen.getByRole("button", { name: /wiki\.test/ }));
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/open/${encodeURIComponent("https://wiki.test/")}`,
    { method: "POST" },
  );
});

test("completes the typed address with the best result's, selected, which typing replaces", () => {
  render(<Overlay />);
  show();
  fireEvent.keyDown(field(), { key: "g" });
  fireEvent.change(field(), { target: { value: "gi" } });
  answer("gi", [], [{ url: "https://www.github.com/arbalete/issues", title: "Issues" }]);
  const input = field() as HTMLInputElement;
  expect(input.value).toBe("github.com");
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 10]);
  expect(picked()).toBe("github.com");
  fireEvent.submit(input);
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/open/${encodeURIComponent("https://www.github.com/")}`,
    { method: "POST" },
  );
  // Typing on keeps completing; a "/" completes the whole address.
  fireEvent.keyDown(input, { key: "/" });
  fireEvent.change(input, { target: { value: "github.com/" } });
  expect(input.value).toBe("github.com/arbalete/issues");
  // Deleting drops the completion, and does not complete again.
  fireEvent.keyDown(input, { key: "Backspace" });
  fireEvent.change(input, { target: { value: "github.com/" } });
  expect(input.value).toBe("github.com/");
  // → keeps it.
  fireEvent.keyDown(input, { key: "a" });
  fireEvent.change(input, { target: { value: "github.com/a" } });
  expect(input.value).toBe("github.com/arbalete/issues");
  fireEvent.keyDown(input, { key: "ArrowRight" });
  expect(input.value).toBe("github.com/arbalete/issues");
});

test("answers to an older search are ignored", () => {
  render(<Overlay />);
  show();
  fireEvent.change(field(), { target: { value: "wik" } });
  fireEvent.change(field(), { target: { value: "wiki" } });
  answer("wik", [], [{ url: "https://old.test/", title: "Old" }]);
  expect(rows()).toEqual(["wiki — Search Google"]);
});

test("⌘L edits the selected tab's address, loaded in that tab", () => {
  render(<Overlay />);
  show("https://example.com/page", "https://ui.test/example.png");
  expect((field() as HTMLInputElement).value).toBe("https://example.com/page");
  // The field shows the page's icon.
  expect(document.querySelector(".overlay-search img")?.getAttribute("src")).toBe(
    "https://ui.test/example.png",
  );
  fireEvent.change(field(), { target: { value: "https://example.com/other" } });
  fireEvent.submit(field());
  expect(fetch).toHaveBeenLastCalledWith(
    `/command/navigate/${encodeURIComponent("https://example.com/other")}`,
    { method: "POST" },
  );
  // Shown again for a new tab, it is empty, with the search icon, and opens one.
  show();
  expect((field() as HTMLInputElement).value).toBe("");
  expect(document.querySelector(".overlay-search img")).toBeNull();
  expect(rows()).toEqual([]);
});

test("hides on escape and when it loses the focus", () => {
  render(<Overlay />);
  fireEvent.keyDown(field(), { key: "Escape" });
  expect(fetch).toHaveBeenLastCalledWith("/command/overlay/hide", { method: "POST" });
  fetch.mockClear();
  fireEvent.blur(field());
  expect(fetch).toHaveBeenLastCalledWith("/command/overlay/hide", { method: "POST" });
});

test("the tab switcher lists the tabs selected last, the picked one highlighted, and fits them", () => {
  render(<Overlay />);
  const tabs = [
    { id: 3, url: "https://a.test/", title: "A", favicon: null },
    { id: 4, url: "https://b.test/", title: "", favicon: null },
  ];
  act(() => {
    dispatchEvent(new CustomEvent("arbalete:switch", { detail: { tabs, picked: 1 } }));
  });
  expect(screen.queryByRole("textbox")).toBeNull();
  const list = screen.getByRole("list", { name: "Recent tabs" });
  expect(list.textContent).toBe("Ahttps://b.test/");
  expect(picked()).toBe("https://b.test/");
  expect(fetch).toHaveBeenLastCalledWith(`/command/overlay/height/${2 * RESULT_ROW + 16}`, {
    method: "POST",
  });
  // Shown again as the Command Bar, it is the field again.
  show();
  expect(field()).toBeDefined();
});

test("tells addresses from searches", () => {
  expect(toUrl("about:blank")).toBe("about:blank");
  expect(toUrl("http://example.com")).toBe("http://example.com");
  expect(toUrl("example.com")).toBe("https://example.com");
  expect(toUrl("docs.example.com:8080/guide")).toBe("https://docs.example.com:8080/guide");
  expect(toUrl("localhost:3000")).toBe("http://localhost:3000");
  expect(toUrl("192.168.1.10")).toBe("https://192.168.1.10");
  expect(toUrl("arc browser")).toBeNull();
  expect(toUrl("weather")).toBeNull();
  expect(toUrl("v1.2")).toBeNull();
});
