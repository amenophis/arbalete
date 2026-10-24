import { type FormEvent, useEffect, useRef, useState } from "react";
import { type Color, firstGrapheme, type Space, send } from "./bridge";
import { keepOnEscape, NameField, Swatches } from "./Profiles";

// One icon per Space, its emoji or a dot, which slides the sidebar to its page with `onPage`. The
// Space whose page shows, if any, is marked aria-current; clicking its icon again opens its editor.
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
  const current = spaces.find((space) => space.id === displayed);
  const editing = current !== undefined && open;
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
        />
      )}
    </>
  );
}

// The emoji field keeps only its first character; emptying it brings the initial back.
function SpaceEditor({ space }: { space: Space }) {
  const { id, title, color } = space;
  const emoji = space.emoji ?? "";
  return (
    <div className="space-editor">
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
// emoji and a color; the app then displays it. Its name takes the keyboard once the page `shown`
// has settled, and leaving it without creating forgets what was typed.
export function NewSpace({ index, shown }: { index: number; shown: boolean }) {
  const [title, setTitle] = useState("");
  const [emoji, setEmoji] = useState("");
  const [color, setColor] = useState<Color>("purple");
  const name = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (shown) {
      name.current?.focus({ preventScroll: true });
      return;
    }
    setTitle("");
    setEmoji("");
    setColor("purple");
  }, [shown]);
  const create = (event: FormEvent) => {
    event.preventDefault();
    if (!title.trim()) return;
    const icon = encodeURIComponent(firstGrapheme(emoji.trim()));
    send(`space/create/${color}/${icon}/${encodeURIComponent(title.trim())}`);
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
