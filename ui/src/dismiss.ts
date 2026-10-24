import { type RefObject, useEffect } from "react";

// While `open`, Escape or a press outside `element` calls `close`, as for a menu or a popover.
export function useDismiss(
  element: RefObject<HTMLElement | null>,
  open: boolean,
  close: () => void,
) {
  useEffect(() => {
    if (!open) return;
    const press = (event: PointerEvent) => {
      if (!element.current?.contains(event.target as Node)) close();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    addEventListener("pointerdown", press);
    addEventListener("keydown", key);
    return () => {
      removeEventListener("pointerdown", press);
      removeEventListener("keydown", key);
    };
  }, [element, open, close]);
}
