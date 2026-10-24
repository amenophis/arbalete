import { useEffect, useState } from "react";
import { send } from "./bridge";

// Sent by the app when the assistant opens (send_sources in app/src/shell.rs): the browsers found
// with their profiles, and the existing profiles to merge into, none on the first launch.
export type Sources = {
  browsers: Browser[];
  profiles: { id: number; name: string }[];
};

type Browser = {
  name: string;
  // Arc's sidebar is imported as Spaces: it has no bookmarks bar to place.
  arc: boolean;
  profiles: { name: string; needsFullDiskAccess: boolean }[];
};

type Bar = "favorites" | "pinnedFolder";

// What to do with a browser profile, by "<browser>/<profile>" index.
type Choice = { checked: boolean; into: number | null; bar: Bar };

// Shown above the page by the app on the first launch and from the menu (show_import in
// app/src/shell.rs), to import other browsers' data. Arc is picked by default: Arbalete users
// most likely come from it. While the app reads the browsers' data, which takes a while for a
// large history, it shows the import running; the app hides it once done.
export function Import() {
  const [sources, setSources] = useState<Sources>();
  const [choices, setChoices] = useState<Record<string, Choice>>({});
  const [importing, setImporting] = useState(false);
  useEffect(() => {
    const listener = (event: Event) => {
      const detail = (event as CustomEvent<Sources>).detail;
      setSources(detail);
      setChoices(initialChoices(detail));
      setImporting(false);
    };
    // Wherever the focus is, even on none of the fields.
    const keyDown = (event: KeyboardEvent) => event.key === "Escape" && send("import/close");
    addEventListener("arbalete:sources", listener);
    addEventListener("keydown", keyDown);
    send("import/ready");
    return () => {
      removeEventListener("arbalete:sources", listener);
      removeEventListener("keydown", keyDown);
    };
  }, []);
  if (!sources) return null;
  const change = (key: string, change: Partial<Choice>) =>
    setChoices({ ...choices, [key]: { ...choices[key], ...change } });
  const picked = Object.entries(choices).filter(([, choice]) => choice.checked);
  return (
    <form
      className="import"
      onSubmit={(event) => {
        event.preventDefault();
        const run = picked.map(([key, { into, bar }]) => {
          const [browser, profile] = key.split("/").map(Number);
          return { browser, profile, into, bar };
        });
        setImporting(true);
        send(`import/run/${encodeURIComponent(JSON.stringify(run))}`);
      }}
    >
      <h1>Import from another browser</h1>
      <p className="import-hint">
        From Arc: its Spaces, favorites, tabs and history. From other browsers: bookmarks and
        history.
      </p>
      <div className="import-sources">
        {sources.browsers.map((browser, b) => (
          <fieldset key={browser.name} disabled={importing}>
            <legend>{browser.name}</legend>
            {browser.profiles.map((profile, p) => {
              const key = `${b}/${p}`;
              const label = `${browser.name} ${profile.name}`;
              const choice = choices[key];
              if (profile.needsFullDiskAccess) {
                return <FullDiskAccess key={key} browser={browser.name} />;
              }
              return (
                <div className="import-row" key={key}>
                  <label>
                    <input
                      type="checkbox"
                      aria-label={`Import ${label}`}
                      checked={choice.checked}
                      onChange={(event) => change(key, { checked: event.target.checked })}
                    />
                    {profile.name}
                  </label>
                  {sources.profiles.length > 0 && (
                    <select
                      aria-label={`Destination of ${label}`}
                      value={choice.into ?? ""}
                      onChange={(event) =>
                        change(key, {
                          into: event.target.value ? Number(event.target.value) : null,
                        })
                      }
                    >
                      <option value="">New profile</option>
                      {sources.profiles.map((target) => (
                        <option key={target.id} value={target.id}>
                          Into {target.name}
                        </option>
                      ))}
                    </select>
                  )}
                  {!browser.arc && (
                    <select
                      aria-label={`Bookmarks bar of ${label}`}
                      value={choice.bar}
                      onChange={(event) => change(key, { bar: event.target.value as Bar })}
                    >
                      <option value="favorites">Bar to Favorites</option>
                      <option value="pinnedFolder">Bar to a pinned folder</option>
                    </select>
                  )}
                </div>
              );
            })}
          </fieldset>
        ))}
      </div>
      <div className="import-actions">
        <button type="button" disabled={importing} onClick={() => send("import/close")}>
          Not now
        </button>
        <button type="submit" disabled={importing || picked.length === 0}>
          {importing ? "Importing…" : "Import"}
        </button>
      </div>
    </form>
  );
}

function initialChoices(sources: Sources): Record<string, Choice> {
  const choices: Record<string, Choice> = {};
  sources.browsers.forEach((browser, b) => {
    browser.profiles.forEach((profile, p) => {
      const checked = browser.arc && !profile.needsFullDiskAccess;
      choices[`${b}/${p}`] = { checked, into: null, bar: "favorites" };
    });
  });
  return choices;
}

// macOS shows Safari's data only to apps the user gave Full Disk Access to.
function FullDiskAccess({ browser }: { browser: string }) {
  return (
    <div className="import-access">
      <p>
        macOS keeps {browser}'s data private. To import it, give Arbalete Full Disk Access in System
        Settings, then open the import again.
      </p>
      <button type="button" onClick={() => send("import/full-disk-access")}>
        Open System Settings
      </button>
    </div>
  );
}
