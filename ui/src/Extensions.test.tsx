import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { ExtensionButtons } from "./Extensions";

const fetch = vi.fn();

beforeEach(() => {
  fetch.mockReset().mockResolvedValue(new Response("{}"));
  vi.stubGlobal("fetch", fetch);
});

afterEach(cleanup);

test("opens an extension's popup under its button", () => {
  const extensions = [
    {
      id: "dark",
      name: "Dark Reader",
      icon: "https://arbalete.ui/extension-icon/1/dark",
      popup: true,
    },
    { id: "quiet", name: "No Popup", icon: null, popup: false },
  ];
  render(<ExtensionButtons extensions={extensions} view="toolbar" />);
  expect(screen.queryByRole("button", { name: "No Popup" })).toBeNull();
  const button = screen.getByRole("button", { name: "Dark Reader" });
  button.getBoundingClientRect = () => ({ x: 10.4, y: 4, width: 28, height: 27.6 }) as DOMRect;
  fireEvent.click(button);
  expect(fetch).toHaveBeenLastCalledWith("/command/extension/popup/dark/toolbar/10/4/28/28", {
    method: "POST",
  });
});
