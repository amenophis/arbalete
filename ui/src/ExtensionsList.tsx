import { useState } from "react";
import { send } from "./bridge";
import { Icon } from "./Icon";
import type { Extension } from "./Site";
import { matches } from "./search";

// What the library shows of an extension (installed_extensions in app/src/shell.rs).
export type InstalledExtension = Extension & {
  enabled: boolean;
  pinned: boolean;
  // It has an options page.
  options: boolean;
  description: string;
  version: string;
  details: {
    // The browser APIs it asks for, and the sites it may read and change.
    permissions: string[];
    sites: string[];
    homepage: string | null;
    manifestVersion: number;
    fromStore: boolean;
    // Loaded from a folder, in developer mode: its path.
    unpacked: string | null;
    // In milliseconds since the epoch.
    installedAt: number | null;
    updatedAt: number | null;
  };
};

const date = (ms: number) =>
  new Date(ms).toLocaleDateString("en", { year: "numeric", month: "short", day: "numeric" });

// Opens a link in a tab.
const open = (url: string) => send(`open/${encodeURIComponent(url)}`);

// The selected extension, in full, like Chromium's extensions page: its icon, name, description
// and switch, its actions, then its version, source, dates, id, permissions and site access.
function ExtensionDetails({ extension }: { extension: InstalledExtension }) {
  const { details } = extension;
  const store = `https://chromewebstore.google.com/detail/${extension.id}`;
  const allSites = (site: string) => site === "<all_urls>" || site === "*://*/*";
  return (
    <section className="extension-details" aria-label={`${extension.name} details`}>
      <header className="extension-header">
        <ExtensionIcon extension={extension} />
        <div className="extension-text">
          <h3 className="extension-title">{extension.name}</h3>
          <span className="extension-description">{extension.description}</span>
        </div>
        <Switch extension={extension} />
      </header>
      <div className="extension-actions">
        <button
          type="button"
          className="extension-action"
          aria-pressed={extension.pinned}
          onClick={() => send(`extension/${extension.pinned ? "unpin" : "pin"}/${extension.id}`)}
        >
          <Icon d="M6 2.5h4M7 2.5v4l-2 2.5h6L9 6.5v-4M8 9v4.5" />
          {extension.pinned ? "Unpin" : "Pin"}
        </button>
        {extension.options && (
          <button
            type="button"
            className="extension-action"
            onClick={() => send(`extension/options/${extension.id}`)}
          >
            <Icon d="M8 5.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
            Options
          </button>
        )}
        {details.fromStore && (
          <button type="button" className="extension-action" onClick={() => open(store)}>
            <Icon d="M9.5 2.5h4v4M13.5 2.5 7.5 8.5M11.5 9.5v4h-9v-9h4" />
            Web Store
          </button>
        )}
        {details.homepage && (
          <button
            type="button"
            className="extension-action"
            onClick={() => details.homepage && open(details.homepage)}
          >
            <Icon d="M9.5 2.5h4v4M13.5 2.5 7.5 8.5M11.5 9.5v4h-9v-9h4" />
            Website
          </button>
        )}
        <button
          type="button"
          className="extension-action extension-remove"
          onClick={() => send(`extension/remove/${extension.id}`)}
        >
          <Icon d="M3 4.5h10M6.5 4.5v-2h3v2M4.5 4.5l.6 9h5.8l.6-9" />
          Remove
        </button>
      </div>
      <dl className="extension-facts">
        <dt>Version</dt>
        <dd>
          {extension.version} · Manifest V{details.manifestVersion}
        </dd>
        <dt>Source</dt>
        <dd>
          {details.fromStore
            ? "Chrome Web Store"
            : details.unpacked
              ? `Unpacked, from ${details.unpacked}`
              : "Not from the Chrome Web Store"}
        </dd>
        {details.installedAt && (
          <>
            <dt>Installed</dt>
            <dd>{date(details.installedAt)}</dd>
          </>
        )}
        {details.updatedAt && details.updatedAt !== details.installedAt && (
          <>
            <dt>Updated</dt>
            <dd>{date(details.updatedAt)}</dd>
          </>
        )}
        <dt>ID</dt>
        <dd className="extension-id">{extension.id}</dd>
      </dl>
      <h4>Permissions</h4>
      <ul className="extension-chips">
        {details.permissions.length ? (
          details.permissions.map((permission) => <li key={permission}>{permission}</li>)
        ) : (
          <li>None</li>
        )}
      </ul>
      <h4>Site access</h4>
      <ul className="extension-chips">
        {details.sites.some(allSites) && <li data-warning>All sites</li>}
        {details.sites
          .filter((site) => !allSites(site))
          .map((site) => (
            <li key={site}>{site}</li>
          ))}
        {details.sites.length === 0 && <li>None</li>}
      </ul>
    </section>
  );
}

function ExtensionIcon({ extension }: { extension: InstalledExtension }) {
  return extension.icon ? (
    <img className="extension-icon" src={extension.icon} alt="" />
  ) : (
    <span className="extension-icon" aria-hidden="true">
      {extension.name.slice(0, 1)}
    </span>
  );
}

// Turns an extension on or off.
function Switch({ extension }: { extension: InstalledExtension }) {
  return (
    <input
      type="checkbox"
      role="switch"
      aria-checked={extension.enabled}
      className="extension-switch"
      aria-label={`Turn ${extension.name} on`}
      checked={extension.enabled}
      onChange={() => send(`extension/${extension.enabled ? "off" : "on"}/${extension.id}`)}
    />
  );
}

// The displayed Space's profile's extensions, in the library, which they fill like the Spaces:
// their list, each one with its switch, and the selected one's details beside it, the first
// one at first. Loading one from a folder takes Chromium's own extensions page, in a window of
// its own: only it can.
export function ExtensionsList({
  extensions,
  query,
}: {
  extensions: InstalledExtension[];
  query: string;
}) {
  const [selected, setSelected] = useState<string>();
  const found = extensions.filter((extension) =>
    matches(query, extension.name, extension.description),
  );
  const shown = found.find((extension) => extension.id === selected) ?? found[0];
  return (
    <div className="extensions">
      <div className="extensions-list">
        <div className="extensions-buttons">
          <button type="button" className="extensions-button" onClick={() => send("site/store")}>
            Add Extensions
          </button>
          <button
            type="button"
            className="extensions-button"
            onClick={() => send("extension/load-unpacked")}
          >
            Load Unpacked…
          </button>
        </div>
        {extensions.length === 0 && <p className="library-empty">No extensions yet</p>}
        {extensions.length > 0 && found.length === 0 && (
          <p className="library-empty">No extension matches</p>
        )}
        <ul>
          {found.map((extension) => (
            <li
              key={extension.id}
              className="extension-row"
              aria-current={extension === shown || undefined}
              data-off={!extension.enabled || undefined}
            >
              <button
                type="button"
                className="extension-select"
                onClick={() => setSelected(extension.id)}
              >
                <ExtensionIcon extension={extension} />
                <span className="extension-name">{extension.name}</span>
                {extension.pinned && (
                  <span role="img" aria-label="Pinned" className="extension-pinned">
                    <Icon d="M6 2.5h4M7 2.5v4l-2 2.5h6L9 6.5v-4M8 9v4.5" size={12} />
                  </span>
                )}
              </button>
              <Switch extension={extension} />
            </li>
          ))}
        </ul>
      </div>
      {shown && <ExtensionDetails extension={shown} />}
    </div>
  );
}
