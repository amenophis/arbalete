import { useState } from "react";
import { firstGrapheme, type Space, send, useRenaming } from "./bridge";
import { keepOnEscape, NameField, Swatches } from "./Profiles";

// One icon per Space, its emoji or its initial, then a button to create one. The displayed Space
// is marked aria-current; clicking its icon again opens its editor, which also opens by itself
// when the app asks to rename the Space it has just created.
export function Spaces({ spaces, displayed }: { spaces: Space[]; displayed?: number }) {
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useRenaming("arbalette:rename-space");
  const current = spaces.find((space) => space.id === displayed);
  const editing = current !== undefined && (open || renaming === current.id);
  return (
    <>
      <ul className="spaces" aria-label="Spaces">
        {spaces.map((space) => {
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
                  setRenaming(null);
                  if (isCurrent) return setOpen(!editing);
                  setOpen(false);
                  send(`space/${space.id}`);
                }}
              >
                {space.emoji ?? firstGrapheme(space.title).toUpperCase()}
              </button>
            </li>
          );
        })}
        <li>
          <button
            type="button"
            className="new-space"
            aria-label="New Space"
            title="New Space"
            onClick={() => send("space/new")}
          >
            +
          </button>
        </li>
      </ul>
      {editing && (
        <SpaceEditor
          // A new name or emoji from the app resets the fields.
          key={`${current.id}:${current.title}:${current.emoji}`}
          space={current}
          renaming={renaming === current.id}
          onRenamed={() => {
            setRenaming(null);
            setOpen(true);
          }}
        />
      )}
    </>
  );
}

// The emoji field keeps only its first character; emptying it brings the initial back.
function SpaceEditor({
  space,
  renaming,
  onRenamed,
}: {
  space: Space;
  renaming: boolean;
  onRenamed: () => void;
}) {
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
          focused={renaming}
          onRename={(value) => send(`space/rename/${id}/${encodeURIComponent(value)}`)}
          onBlur={() => renaming && onRenamed()}
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
