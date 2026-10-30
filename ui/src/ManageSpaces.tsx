import { type PointerEvent, useLayoutEffect, useRef, useState } from "react";
import { COLORS, type Glance, type Profile, type Space, send } from "./bridge";
import { swallowClick, THRESHOLD } from "./drag";
import { Icon } from "./Icon";
import { ROW_SLIDE, rowRects, slideFrom } from "./motion";
import { NameField } from "./Profiles";
import { SpaceEditor } from "./Spaces";

// How long a click on a Space's name waits for a second one, which renames it instead.
export const RENAME_CLICK_MS = 250;

// Where a dragged card lands: before the card `id`, or after it.
export type CardDrop = { id: number; after: boolean };

// The command moving the Space `id` to `drop` among the Spaces in `order`, or null when it would
// stay in place. The app counts the index once the Space has left its place.
export function spaceMoveCommand(order: number[], id: number, drop: CardDrop): string | null {
  const from = order.indexOf(id);
  const to = order.indexOf(drop.id) + (drop.after ? 1 : 0);
  const index = to > from ? to - 1 : to;
  return index === from ? null : `space/move/${id}/${index}`;
}

// One card per Space, side by side in sidebar order. A card shows its
// Space's color, name, profile and pinned tabs; clicking it shows the Space, dragging it by its
// handle moves it among the others. The "+" after them creates a Space.
export function ManageSpaces({
  spaces,
  profiles,
  onShow,
  onNewSpace,
}: {
  spaces: Space[];
  profiles: Profile[];
  onShow: (index: number) => void;
  onNewSpace: () => void;
}) {
  const list = useRef<HTMLUListElement>(null);
  // Set on a drop, until the app answers with the new order: the cards' places before it.
  const dropped = useRef<{ spaces: Space[]; before: Map<string, DOMRect> } | null>(null);
  useLayoutEffect(() => {
    const drop = dropped.current;
    if (!drop || drop.spaces === spaces || !list.current) return;
    dropped.current = null;
    for (const card of list.current.querySelectorAll(".dragged")) card.classList.remove("dragged");
    slideFrom(drop.before, list.current, ROW_SLIDE);
  }, [spaces]);
  const order = spaces.map((space) => space.id);
  const onDrag = (event: PointerEvent, id: number) => {
    if (event.button !== 0 || !list.current) return;
    dragCard(event, list.current, (drop) => {
      const command = drop && spaceMoveCommand(order, id, drop);
      if (!command) return false;
      dropped.current = { spaces, before: rowRects(list.current as HTMLElement) };
      send(command);
      return true;
    });
  };
  return (
    <ul className="space-cards" ref={list} onWheel={scrollSideways}>
      {spaces.map((space) => (
        <SpaceCard
          key={space.id}
          space={space}
          profile={profiles.find((profile) => profile.id === space.profile)}
          onShow={() => onShow(order.indexOf(space.id))}
          onDrag={(event) => onDrag(event, space.id)}
        />
      ))}
      <li className="space-add">
        <button type="button" aria-label="New Space" title="New Space" onClick={onNewSpace}>
          <Icon d="M8 3v10M3 8h10" />
        </button>
      </li>
    </ul>
  );
}

// A vertical wheel over the cards, but over a list of pinned tabs that scrolls, scrolls them
// sideways: a horizontal swipe pages between the library and the Spaces.
function scrollSideways(event: React.WheelEvent<HTMLUListElement>) {
  const pinned = (event.target as Element).closest(".space-card-pinned");
  if (pinned && pinned.scrollHeight > pinned.clientHeight) return;
  if (Math.abs(event.deltaY) > Math.abs(event.deltaX))
    event.currentTarget.scrollLeft += event.deltaY;
}

function SpaceCard({
  space,
  profile,
  onShow,
  onDrag,
}: {
  space: Space;
  profile?: Profile;
  onShow: () => void;
  onDrag: (event: PointerEvent) => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [editing, setEditing] = useState(false);
  const click = useRef<ReturnType<typeof setTimeout>>(undefined);
  const { id, title } = space;
  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: the whole card shows its Space; its name button does from the keyboard.
    <li
      className="space-card"
      data-id={id}
      style={{ "--card-color": space.theme.colors[0] ?? COLORS.gray } as React.CSSProperties}
      onClick={(event) => {
        if (!(event.target as Element).closest("button, input, fieldset")) onShow();
      }}
    >
      <div className="space-card-header">
        <span className="space-card-icon" aria-hidden="true">
          {space.emoji ?? <span className="space-dot" />}
        </span>
        {renaming ? (
          <NameField
            label="Space name"
            name={title}
            focused
            onRename={(value) => send(`space/rename/${id}/${encodeURIComponent(value)}`)}
            onBlur={() => setRenaming(false)}
          />
        ) : (
          // A click waits to know it does not start a double click, which renames the Space.
          <button
            type="button"
            className="space-card-name"
            aria-label={`Show ${title}`}
            onClick={(event) => {
              clearTimeout(click.current);
              if (event.detail > 1) return;
              if (event.detail === 0) onShow();
              else click.current = setTimeout(onShow, RENAME_CLICK_MS);
            }}
            onDoubleClick={() => {
              clearTimeout(click.current);
              setRenaming(true);
            }}
          >
            {title}
          </button>
        )}
        <button
          type="button"
          className="space-card-edit"
          aria-label={`Edit ${title}`}
          title="Edit name, icon and color"
          aria-expanded={editing}
          onClick={() => setEditing(!editing)}
        >
          <Icon d="M10.5 2.8 13.2 5.5 6 12.7l-3.2.5.5-3.2zM9.2 4.1l2.7 2.7" />
        </button>
      </div>
      <button
        type="button"
        className="space-card-profile"
        title="Change profile"
        onClick={() => send(`menu/space-profile/${id}`)}
      >
        {profile?.name}
      </button>
      {editing && <SpaceEditor key={`${title}:${space.emoji}`} space={space} />}
      <ul className="space-card-pinned" aria-label={`Pinned in ${title}`}>
        {space.pinned.map((item) => (
          <PinnedGlance key={item.id} item={item} />
        ))}
      </ul>
      <div className="space-card-footer">
        <button
          type="button"
          className="space-card-handle"
          aria-label={`Move ${title}`}
          title="Drag to move"
          onPointerDown={onDrag}
        >
          <Icon d="M6 4h.01M10 4h.01M6 8h.01M10 8h.01M6 12h.01M10 12h.01" />
        </button>
        <button
          type="button"
          className="space-card-more"
          aria-label={`${title} options`}
          title="Space options"
          onClick={() => send(`menu/space/${id}`)}
        >
          <Icon d="M4 8h.01M8 8h.01M12 8h.01" />
        </button>
      </div>
    </li>
  );
}

function PinnedGlance({ item }: { item: Glance }) {
  if ("folder" in item) {
    return (
      <li className="glance">
        <Icon d="M2.5 4.5a1 1 0 0 1 1-1h2.8l1.4 1.4h4.8a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z" />
        <span className="title">{item.title}</span>
      </li>
    );
  }
  return (
    <li className="glance">
      {item.favicon ? (
        <img className="favicon" src={item.favicon} alt="" />
      ) : (
        <span className="favicon placeholder" />
      )}
      <span className="title">{item.title || item.url}</span>
    </li>
  );
}

// A press on a card's handle that moves a few pixels drags the card: it stays in place, dimmed,
// while a copy follows the pointer and a vertical line shows where it would land. Escape cancels.
// `onDrop` is told where it lands, or null outside the cards, and whether it moves.
function dragCard(
  event: PointerEvent,
  list: HTMLElement,
  onDrop: (drop: CardDrop | null) => boolean,
) {
  const card = (event.currentTarget as Element).closest<HTMLElement>("li[data-id]");
  if (!card) return;
  const start = card.getBoundingClientRect();
  const grab = { x: event.clientX - start.left, y: event.clientY - start.top };
  const origin = { x: event.clientX, y: event.clientY };
  let ghost: HTMLElement | null = null;
  let drop: CardDrop | null = null;
  // Outside the pages, which move with a transform: fixed in them would be fixed to them.
  const line = document.createElement("div");
  line.className = "space-drop-line";

  const locate = (x: number, y: number) => {
    const area = list.getBoundingClientRect();
    const cards = [...list.querySelectorAll<HTMLElement>("li[data-id]")];
    if (y < area.top || y > area.bottom || cards.length === 0) {
      drop = null;
      line.style.cssText = "";
      return;
    }
    const rects = cards.map((element) => element.getBoundingClientRect());
    const before = rects.findIndex((rect) => x < rect.left + rect.width / 2);
    const after = before === -1;
    const target = after ? cards.length - 1 : before;
    drop = { id: Number(cards[target].dataset.id), after };
    const rect = rects[target];
    // Halfway in the gap between two cards.
    const gap = rects.length > 1 ? rects[1].left - rects[0].right : 8;
    const left = (after ? rect.right : rect.left) + (after ? gap : -gap) / 2;
    line.style.cssText = `display: block; left: ${left - 0.75}px; top: ${rect.top}px; height: ${rect.height}px`;
  };
  const move = (event: globalThis.PointerEvent) => {
    const { clientX: x, clientY: y } = event;
    if (!ghost) {
      if (Math.hypot(x - origin.x, y - origin.y) < THRESHOLD) return;
      addEventListener("keydown", onKey, true);
      card.classList.add("dragged");
      ghost = document.createElement("div");
      ghost.className = "drag-ghost card-ghost";
      ghost.style.setProperty("--width", `${start.width}px`);
      const copy = card.cloneNode(true) as HTMLElement;
      copy.classList.remove("dragged");
      copy.style.height = `${start.height}px`;
      ghost.append(copy);
      document.body.append(ghost, line);
    }
    ghost.style.transform = `translate(${x - grab.x}px, ${y - grab.y}px)`;
    locate(x, y);
  };
  const finish = (commit: boolean) => {
    removeEventListener("pointermove", move);
    removeEventListener("pointerup", up);
    removeEventListener("pointercancel", cancel);
    removeEventListener("keydown", onKey, true);
    if (!ghost) return;
    ghost.remove();
    line.remove();
    // The card stays dimmed until the app answers with the new order.
    if (!(commit && onDrop(drop))) card.classList.remove("dragged");
  };
  const up = () => {
    if (ghost) swallowClick();
    finish(true);
  };
  const cancel = () => finish(false);
  const onKey = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    addEventListener("pointerup", swallowClick, { once: true });
    finish(false);
  };
  addEventListener("pointermove", move);
  addEventListener("pointerup", up);
  addEventListener("pointercancel", cancel);
}
