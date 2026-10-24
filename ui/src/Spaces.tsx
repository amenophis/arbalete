import { type FormEvent, type Ref, useEffect, useRef, useState } from "react";
import { type Color, firstGrapheme, type Space, send, useAppEvent } from "./bridge";
import { keepOnEscape, NameField, Swatches } from "./Profiles";

// One icon per Space, its emoji or a dot, which slides the sidebar to its page with `onPage`. The
// Space whose page shows, if any, is marked aria-current; clicking its icon again opens its editor,
// which the Space's menu also opens on one of its fields.
export function Spaces({
  spaces,
  displayed,
  onPage,
}: {
  spaces: Space[];
  displayed?: number;
  onPage: (page: number) => void;
}) {
  const [open, setOpen] = useState(false);
  // A new request each time, so that asking for the same field again focuses it again.
  const [focus, setFocus] = useState<{ field: Field }>();
  const current = spaces.find((space) => space.id === displayed);
  const editing = current !== undefined && open;
  useAppEvent<{ space: number; field: Field }>("arbalete:edit-space", (detail) => {
    if (detail.space !== displayed) return;
    setOpen(true);
    setFocus({ field: detail.field });
  });
  const editor = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const selector = {
      emoji: ".emoji",
      name: "input:not(.emoji)",
      color: ".swatch[aria-pressed=true]",
    };
    if (focus) editor.current?.querySelector<HTMLElement>(selector[focus.field])?.focus();
  }, [focus]);
  return (
    <>
      <ul className="spaces" aria-label="Spaces">
        {spaces.map((space, index) => {
          const isCurrent = space.id === displayed;
          return (
            <li key={space.id}>
              <button
                type="button"
                className="space"
                aria-label={space.title}
                title={space.title}
                aria-current={isCurrent ? "true" : undefined}
                aria-expanded={isCurrent ? editing : undefined}
                onClick={() => {
                  if (isCurrent) return setOpen(!editing);
                  setOpen(false);
                  onPage(index);
                }}
              >
                {space.emoji ?? <span className="dot" />}
              </button>
            </li>
          );
        })}
      </ul>
      {editing && (
        <SpaceEditor
          // A new name or emoji from the app resets the fields.
          key={`${current.id}:${current.title}:${current.emoji}`}
          space={current}
          ref={editor}
        />
      )}
    </>
  );
}

type Field = "emoji" | "name" | "color";

// The emoji field keeps only its first character; emptying it brings the initial back.
export function SpaceEditor({ space, ref }: { space: Space; ref?: Ref<HTMLDivElement> }) {
  const { id, title, color } = space;
  const emoji = space.emoji ?? "";
  return (
    <div className="space-editor" ref={ref}>
      <div className="space-fields">
        <input
          className="emoji"
          aria-label="Space emoji"
          defaultValue={emoji}
          placeholder={firstGrapheme(title).toUpperCase()}
          onFocus={(event) => event.currentTarget.select()}
          onKeyDown={keepOnEscape(emoji)}
          onBlur={(event) => {
            const value = firstGrapheme(event.currentTarget.value.trim());
            event.currentTarget.value = value;
            if (value !== emoji) send(`space/emoji/${id}/${encodeURIComponent(value)}`);
          }}
        />
        <NameField
          label="Space name"
          name={title}
          onRename={(value) => send(`space/rename/${id}/${encodeURIComponent(value)}`)}
        />
      </div>
      <Swatches
        label={`Color of ${title}`}
        color={color}
        onPick={(option) => send(`space/color/${id}/${option}`)}
      />
    </div>
  );
}

// The page after the last Space creates one, in the displayed Space's profile, from a name, an
// emoji and a color; the app then displays it, with the tab whose menu opened the page moved into
// it. Its name takes the keyboard once the page `shown` has settled, and leaving it without
// creating forgets what was typed and the tab.
export function NewSpace({ index, shown }: { index: number; shown: boolean }) {
  const [title, setTitle] = useState("");
  const [emoji, setEmoji] = useState("");
  const [color, setColor] = useState<Color>("purple");
  const [tab, setTab] = useState<number | null>(null);
  useAppEvent("arbalete:new-space", setTab);
  const name = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (shown) {
      name.current?.focus({ preventScroll: true });
      return;
    }
    setTitle("");
    setEmoji("");
    setColor("purple");
    setTab(null);
  }, [shown]);
  const create = (event: FormEvent) => {
    event.preventDefault();
    if (!title.trim()) return;
    const icon = encodeURIComponent(firstGrapheme(emoji.trim()));
    const moved = tab === null ? "" : `/${tab}`;
    send(`space/create/${color}/${icon}/${encodeURIComponent(title.trim())}${moved}`);
  };
  return (
    <form
      className="end-page new-space-page"
      aria-label="New Space"
      inert={!shown}
      style={{ left: `${index * 100}%` }}
      onSubmit={create}
    >
      <h2>New Space</h2>
      <div className="space-fields">
        <input
          className="emoji"
          aria-label="New Space emoji"
          value={emoji}
          placeholder={firstGrapheme(title).toUpperCase()}
          onChange={(event) => setEmoji(event.currentTarget.value)}
          onBlur={() => setEmoji(firstGrapheme(emoji.trim()))}
        />
        <input
          aria-label="New Space name"
          ref={name}
          value={title}
          placeholder="Name"
          onChange={(event) => setTitle(event.currentTarget.value)}
        />
      </div>
      <Swatches label="Color of the new Space" color={color} onPick={setColor} />
      <button type="submit" className="create-space" disabled={!title.trim()}>
        Create
      </button>
    </form>
  );
}
