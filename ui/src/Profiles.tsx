import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { COLORS, type Color, type Profile, send, useRenaming } from "./bridge";

// A button at the bottom of the sidebar opens the list of profiles, to create one, rename one or
// pick its color. It opens by itself when the app asks to rename the profile it has just created.
export function Profiles({ profiles }: { profiles: Profile[] }) {
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useRenaming("arbalette:rename-profile");
  const shown = open || renaming !== null;
  return (
    <div className="profiles">
      {shown && (
        <ul aria-label="Profiles">
          {profiles.map((profile) => (
            <ProfileRow
              // A new name from the app resets the field.
              key={`${profile.id}:${profile.name}`}
              profile={profile}
              renaming={profile.id === renaming}
              onRenamed={() => {
                setRenaming(null);
                setOpen(true);
              }}
            />
          ))}
          <li>
            <button type="button" className="new-profile" onClick={() => send("profile/new")}>
              + New profile
            </button>
          </li>
        </ul>
      )}
      <button
        type="button"
        className="profiles-toggle"
        aria-expanded={shown}
        onClick={() => {
          setRenaming(null);
          setOpen(!shown);
        }}
      >
        Profiles
      </button>
    </div>
  );
}

function ProfileRow({
  profile,
  renaming,
  onRenamed,
}: {
  profile: Profile;
  renaming: boolean;
  onRenamed: () => void;
}) {
  const { id, name, color } = profile;
  return (
    <li className="profile">
      <NameField
        label="Profile name"
        name={name}
        focused={renaming}
        onRename={(value) => send(`profile/rename/${id}/${encodeURIComponent(value)}`)}
        onBlur={() => renaming && onRenamed()}
      />
      <Swatches
        label={`Color of ${name}`}
        color={color}
        onPick={(option) => send(`profile/color/${id}/${option}`)}
      />
    </li>
  );
}

// Enter and leaving the field save the name, unless it is blank; Escape keeps the old one.
export function NameField({
  label,
  name,
  focused,
  onRename,
  onBlur,
}: {
  label: string;
  name: string;
  focused: boolean;
  onRename: (name: string) => void;
  onBlur: () => void;
}) {
  // The field may show before the app asks to rename it: autoFocus would come too late.
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (focused) input.current?.focus();
  }, [focused]);
  return (
    <input
      aria-label={label}
      ref={input}
      defaultValue={name}
      onFocus={(event) => event.currentTarget.select()}
      onKeyDown={keepOnEscape(name)}
      onBlur={(event) => {
        const value = event.currentTarget.value.trim();
        if (value && value !== name) onRename(value);
        else event.currentTarget.value = name;
        onBlur();
      }}
    />
  );
}

// Enter leaves a field, which saves it; Escape first puts `value` back.
export function keepOnEscape(value: string) {
  return (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") event.currentTarget.value = value;
    if (event.key === "Escape" || event.key === "Enter") event.currentTarget.blur();
  };
}

export function Swatches({
  label,
  color,
  onPick,
}: {
  label: string;
  color: Color;
  onPick: (color: Color) => void;
}) {
  return (
    <fieldset className="swatches" aria-label={label}>
      {(Object.keys(COLORS) as Color[]).map((option) => (
        <button
          key={option}
          type="button"
          className="swatch"
          aria-label={option}
          aria-pressed={option === color}
          style={{ background: COLORS[option] }}
          onClick={() => onPick(option)}
        />
      ))}
    </fieldset>
  );
}
