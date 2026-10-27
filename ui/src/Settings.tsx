import { useEffect, useState } from "react";
import { COLORS, type Color, send } from "./bridge";

// Sent by the app when the window opens and after every change (send_settings in
// app/src/shell.rs).
export type SettingsDetail = {
  settings: { warnBeforeQuit: boolean; restoreSession: boolean };
  engines: { id: string; name: string }[];
  profiles: { id: number; name: string; color: Color; searchEngine: string }[];
};

type Section = "general" | "profiles" | "shortcuts";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "general", label: "General" },
  { id: "profiles", label: "Profiles" },
  { id: "shortcuts", label: "Shortcuts" },
];

// Every shortcut, as the app's menus and keyboard monitor define them (app/src/mac.rs).
export const SHORTCUTS: [string, [string, string][]][] = [
  [
    "Tabs",
    [
      ["New Tab", "⌘T"],
      ["Edit Address", "⌘L"],
      ["Close Tab", "⌘W"],
      ["Reopen Closed Tab", "⇧⌘T"],
      ["Pin or Unpin Tab", "⌘D"],
      ["Copy URL", "⇧⌘C"],
      ["Next Tab", "⌥⌘↓ or ⌘}"],
      ["Previous Tab", "⌥⌘↑ or ⌘{"],
      ["Switch Between Recent Tabs", "⌃⇥, ⌃⇧⇥"],
      ["Favorite 1 to 8", "⌘1 … ⌘8"],
      ["Last Favorite", "⌘9"],
      ["Clear Today", "⇧⌘K"],
    ],
  ],
  [
    "Spaces",
    [
      ["Next Space", "⌥⌘→"],
      ["Previous Space", "⌥⌘←"],
      ["Space 1 to 9", "⌃1 … ⌃9"],
    ],
  ],
  [
    "Page",
    [
      ["Back", "⌘["],
      ["Forward", "⌘]"],
      ["Reload", "⌘R"],
      ["Force Reload", "⇧⌘R"],
      ["Stop Loading", "⌘."],
      ["Find", "⌘F"],
      ["Find Next", "⌘G"],
      ["Find Previous", "⇧⌘G"],
      ["Actual Size", "⌘0"],
      ["Zoom In", "⌘="],
      ["Zoom Out", "⌘-"],
      ["Print", "⌘P"],
    ],
  ],
  [
    "Developer",
    [
      ["View Source", "⌥⌘U"],
      ["Developer Tools", "⌥⌘I or F12"],
    ],
  ],
  [
    "Window",
    [
      ["Hide or Show Sidebar", "⌘S"],
      ["Library", "⇧⌘L"],
      ["History", "⌘Y"],
      ["Downloads", "⇧⌘J"],
      ["Settings", "⌘,"],
      ["Minimize", "⌘M"],
      ["Quit", "⌘Q"],
    ],
  ],
];

// The settings window's page, which the app opens from Arbalete › Settings… (show_settings in
// app/src/shell.rs): the app's own settings, each profile's search engine, and every shortcut.
// Escape closes the window, like its close button.
export function Settings() {
  const [detail, setDetail] = useState<SettingsDetail>();
  const [section, setSection] = useState<Section>("general");
  useEffect(() => {
    const listener = (event: Event) => setDetail((event as CustomEvent<SettingsDetail>).detail);
    // Wherever the focus is, even on none of the fields.
    const keyDown = (event: KeyboardEvent) => event.key === "Escape" && send("settings/hide");
    addEventListener("arbalete:settings", listener);
    addEventListener("keydown", keyDown);
    send("settings/ready");
    return () => {
      removeEventListener("arbalete:settings", listener);
      removeEventListener("keydown", keyDown);
    };
  }, []);
  if (!detail) return null;
  const label = SECTIONS.find(({ id }) => id === section)?.label;
  return (
    <div className="settings">
      <header>
        <div role="tablist" aria-label="Settings sections">
          {SECTIONS.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={id === section}
              onClick={() => setSection(id)}
            >
              {label}
            </button>
          ))}
        </div>
      </header>
      <div className="settings-panel" role="tabpanel" aria-label={label}>
        {section === "general" && <General settings={detail.settings} />}
        {section === "profiles" && <Profiles detail={detail} />}
        {section === "shortcuts" && <Shortcuts />}
      </div>
    </div>
  );
}

function General({ settings }: { settings: SettingsDetail["settings"] }) {
  const toggle = (key: string, on: boolean) => send(`settings/${key}/${on}`);
  return (
    <>
      <label className="settings-row">
        <input
          type="checkbox"
          checked={settings.warnBeforeQuit}
          onChange={(event) => toggle("warn-before-quit", event.currentTarget.checked)}
        />
        <span>
          Warn before quitting
          <small>⌘Q asks before closing Arbalete.</small>
        </span>
      </label>
      <label className="settings-row">
        <input
          type="checkbox"
          checked={settings.restoreSession}
          onChange={(event) => toggle("restore-session", event.currentTarget.checked)}
        />
        <span>
          Restore the previous session
          <small>
            Reopens the “Today” tabs left open at the last quit. Pinned tabs and favorites always
            stay.
          </small>
        </span>
      </label>
    </>
  );
}

function Profiles({ detail }: { detail: SettingsDetail }) {
  return (
    <ul className="settings-profiles">
      {detail.profiles.map((profile) => (
        <li key={profile.id} className="settings-row">
          <span className="settings-dot" style={{ background: COLORS[profile.color] }} />
          <span className="settings-name">{profile.name}</span>
          <label>
            Search engine{" "}
            <select
              aria-label={`${profile.name} search engine`}
              value={profile.searchEngine}
              onChange={(event) =>
                send(`profile/search/${profile.id}/${event.currentTarget.value}`)
              }
            >
              {detail.engines.map((engine) => (
                <option key={engine.id} value={engine.id}>
                  {engine.name}
                </option>
              ))}
            </select>
          </label>
        </li>
      ))}
    </ul>
  );
}

function Shortcuts() {
  return (
    <div className="settings-shortcuts">
      {SHORTCUTS.map(([group, shortcuts]) => (
        <section key={group} aria-label={group}>
          <h3>{group}</h3>
          <dl>
            {shortcuts.map(([action, keys]) => (
              <div key={action}>
                <dt>{action}</dt>
                <dd>{keys}</dd>
              </div>
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}
