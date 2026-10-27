import { useEffect, useState } from "react";
import { COLORS, type Color, send } from "./bridge";
import { Icon } from "./Icon";

// Sent by the app when the window opens and after every change (send_settings in
// app/src/shell.rs).
export type SettingsDetail = {
  settings: { warnBeforeQuit: boolean; restoreSession: boolean };
  engines: { id: string; name: string }[];
  profiles: { id: number; name: string; color: Color; searchEngine: string }[];
};

type Section = "general" | "profiles" | "shortcuts";

const SECTIONS: { id: Section; label: string; icon: string }[] = [
  {
    id: "general",
    label: "General",
    icon: "M8 10.2a2.2 2.2 0 1 0 0-4.4 2.2 2.2 0 0 0 0 4.4zM8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1",
  },
  {
    id: "profiles",
    label: "Profiles",
    icon: "M8 7.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM3 13.5c0-2.5 2.2-4 5-4s5 1.5 5 4",
  },
  {
    id: "shortcuts",
    label: "Shortcuts",
    icon: "M2 4.5h12v7H2zM4.5 7h.01M6.5 7h.01M8.5 7h.01M10.5 7h.01M5 9.5h6",
  },
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
// Like Arc's, the window has no title bar of its own: the page draws the picked section's name
// beside the window buttons, then a toolbar of sections, which both move the window. Escape
// closes the window, like its close button.
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
        <h1>{label}</h1>
        <div role="tablist" aria-label="Settings sections">
          {SECTIONS.map(({ id, label, icon }) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={id === section}
              onClick={() => setSection(id)}
            >
              <Icon d={icon} size={24} />
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
