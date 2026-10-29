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

// What Chromium's extensions page tells of an extension: its id, version, where it came from,
// when, and what it may do.
function ExtensionDetails({ extension }: { extension: InstalledExtension }) {
  const { details } = extension;
  const store = `https://chromewebstore.google.com/detail/${extension.id}`;
  return (
    <dl className="extension-details">
      <dt>ID</dt>
      <dd className="extension-id">{extension.id}</dd>
      <dt>Version</dt>
      <dd>
        {extension.version} (Manifest V{details.manifestVersion})
      </dd>
      <dt>Source</dt>
      <dd>
        {details.fromStore ? (
          <button type="button" className="extension-link" onClick={() => open(store)}>
            Chrome Web Store
          </button>
        ) : details.unpacked ? (
          `Unpacked, from ${details.unpacked}`
        ) : (
          "Not from the Chrome Web Store"
        )}
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
      <dt>Permissions</dt>
      <dd>{details.permissions.length ? details.permissions.join(", ") : "None"}</dd>
      <dt>Site access</dt>
      <dd>{details.sites.length ? details.sites.join(", ") : "None"}</dd>
      {details.homepage && (
        <>
          <dt>Website</dt>
          <dd>
            <button
              type="button"
              className="extension-link"
              onClick={() => details.homepage && open(details.homepage)}
            >
              {details.homepage}
            </button>
          </dd>
        </>
      )}
    </dl>
  );
}

// The displayed Space's profile's extensions, in the library: each one turned on or off, pinned
// by the address, its options page, its removal, and its details, which a click on it shows. Loading one from a folder takes Chromium's own
// extensions page, in a window of its own: only it can.
export function ExtensionsList({
  extensions,
  query,
}: {
  extensions: InstalledExtension[];
  query: string;
}) {
  const [expanded, setExpanded] = useState<string>();
  const found = extensions.filter((extension) =>
    matches(query, extension.name, extension.description),
  );
  return (
    <div className="extensions-list">
      <div className="extensions-actions">
        <button type="button" className="extensions-action" onClick={() => send("site/store")}>
          Add Extensions
        </button>
        <button
          type="button"
          className="extensions-action"
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
          <li key={extension.id} className="extension-item">
            <div className="extension-row" data-off={!extension.enabled || undefined}>
              {extension.icon ? (
                <img className="extension-icon" src={extension.icon} alt="" />
              ) : (
                <span className="extension-icon">{extension.name.slice(0, 1)}</span>
              )}
              <button
                type="button"
                className="extension-text"
                aria-expanded={expanded === extension.id}
                onClick={() => setExpanded(expanded === extension.id ? undefined : extension.id)}
              >
                <span className="extension-title">
                  {extension.name} <span className="extension-version">{extension.version}</span>
                </span>
                <span className="extension-description">{extension.description}</span>
              </button>
              <button
                type="button"
                className="extension-control"
                aria-label={extension.pinned ? `Unpin ${extension.name}` : `Pin ${extension.name}`}
                title={extension.pinned ? "Unpin" : "Pin"}
                aria-pressed={extension.pinned}
                onClick={() =>
                  send(`extension/${extension.pinned ? "unpin" : "pin"}/${extension.id}`)
                }
              >
                <Icon d="M6 2.5h4M7 2.5v4l-2 2.5h6L9 6.5v-4M8 9v4.5" />
              </button>
              {extension.options && (
                <button
                  type="button"
                  className="extension-control"
                  aria-label={`${extension.name} options`}
                  title="Options"
                  onClick={() => send(`extension/options/${extension.id}`)}
                >
                  <Icon d="M8 5.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
                </button>
              )}
              <button
                type="button"
                className="extension-control"
                aria-label={`Remove ${extension.name}`}
                title="Remove"
                onClick={() => send(`extension/remove/${extension.id}`)}
              >
                <Icon d="M3 4.5h10M6.5 4.5v-2h3v2M4.5 4.5l.6 9h5.8l.6-9" />
              </button>
              <input
                type="checkbox"
                role="switch"
                aria-checked={extension.enabled}
                className="extension-switch"
                aria-label={`Turn ${extension.name} on`}
                checked={extension.enabled}
                onChange={() =>
                  send(`extension/${extension.enabled ? "off" : "on"}/${extension.id}`)
                }
              />
            </div>
            {expanded === extension.id && <ExtensionDetails extension={extension} />}
          </li>
        ))}
      </ul>
    </div>
  );
}
