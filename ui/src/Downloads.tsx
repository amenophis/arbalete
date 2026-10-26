import { useState } from "react";
import { send, useAppEvent } from "./bridge";
import { Icon } from "./Icon";
import { matches } from "./ManageSpaces";

export type Download = {
  id: number;
  name: string;
  // The address the file came from.
  url: string;
  received: number;
  // 0 while the size is unknown.
  total: number;
  // Bytes per second.
  speed: number;
  state: "inProgress" | "complete" | "canceled" | "failed";
  // Milliseconds since the Unix epoch.
  startedAt: number;
  // The file was deleted or moved since.
  missing: boolean;
};

// How long a finished download stays at the bottom of the sidebar, and how many show there.
export const RECENT_MS = 24 * 60 * 60 * 1000;
export const RECENT_COUNT = 3;

// The downloads, kept across sessions, as the app sends them (send_downloads in
// app/src/shell.rs), most recent last.
export function useDownloads(): Download[] {
  const [downloads, setDownloads] = useState<Download[]>([]);
  useAppEvent<Download[]>("arbalete:downloads", setDownloads);
  return downloads;
}

// The downloads in progress and the ones of the last day at the bottom of the sidebar, the
// library listing them all.
export function Downloads({ downloads, now }: { downloads: Download[]; now: number }) {
  const recent = downloads
    .filter((download) => download.state === "inProgress" || now - download.startedAt < RECENT_MS)
    .slice(-RECENT_COUNT);
  if (recent.length === 0) return null;
  return (
    <ul className="downloads" aria-label="Downloads">
      {recent.map((download) => (
        <DownloadRow key={download.id} download={download} />
      ))}
    </ul>
  );
}

// Every download, the most recent first, grouped by day, under the library's search field.
export function DownloadsList({
  downloads,
  query,
  now,
}: {
  downloads: Download[];
  query: string;
  now: number;
}) {
  if (downloads.length === 0) return <p className="library-empty">No download yet</p>;
  const found = downloads
    .filter((download) => matches(query, download.name, download.url))
    .reverse();
  if (found.length === 0) return <p className="library-empty">No download matches</p>;
  const days = new Map<string, Download[]>();
  for (const download of found) {
    const name = day(download.startedAt, now);
    days.set(name, [...(days.get(name) ?? []), download]);
  }
  return (
    <div className="downloads-list">
      {[...days].map(([name, list]) => (
        <section key={name} aria-label={name}>
          <h3>{name}</h3>
          <ul>
            {list.map((download) => (
              <DownloadRow key={download.id} download={download} source />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

// A finished download opens on a click, and shows in the Finder from its button; the × cancels
// one in progress, or takes another off the list. `source` adds the site it came from.
function DownloadRow({ download, source = false }: { download: Download; source?: boolean }) {
  const available = download.state === "complete" && !download.missing;
  const running = download.state === "inProgress";
  const from = source && host(download.url);
  return (
    <li className="tab-row download">
      <button
        type="button"
        className="tab"
        disabled={!available}
        title={download.name}
        onClick={() => send(`download/open/${download.id}`)}
      >
        <span className="favicon">
          <Icon d="M8 2.5v8M4.5 7 8 10.5 11.5 7M3 13.5h10" />
        </span>
        <span className="download-text">
          <span className="title">{download.name}</span>
          <span className="download-detail">
            {from ? `${detail(download)} · ${from}` : detail(download)}
          </span>
        </span>
        {running && download.total > 0 && (
          <progress value={download.received} max={download.total} aria-label="Progress" />
        )}
      </button>
      {available && (
        <button
          type="button"
          className="close reveal"
          aria-label="Show in Finder"
          onClick={() => send(`download/show/${download.id}`)}
        >
          <Icon d="M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3 13.5 13.5" size={14} />
        </button>
      )}
      <button
        type="button"
        className="close"
        aria-label={running ? "Cancel download" : "Remove from list"}
        onClick={() => send(`download/${running ? "cancel" : "remove"}/${download.id}`)}
      >
        ×
      </button>
    </li>
  );
}

// "12.3 / 45.6 MB · 8 s left" while downloading, then what became of it.
export function detail(download: Download): string {
  switch (download.state) {
    case "complete":
      return download.missing ? "Deleted" : size(download.received);
    case "canceled":
      return "Canceled";
    case "failed":
      return "Failed";
  }
  if (!download.total) return size(download.received);
  const [unit, divisor] = unitOf(download.total);
  const progress = `${amount(download.received, divisor)} / ${amount(download.total, divisor)} ${unit}`;
  if (!download.speed) return progress;
  const left = Math.ceil((download.total - download.received) / download.speed);
  return `${progress} · ${duration(left)} left`;
}

// "Today", "Yesterday", then the date.
export function day(time: number, now: number): string {
  const start = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.round(
    (start(new Date(now)).getTime() - start(new Date(time)).getTime()) / 86_400_000,
  );
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  return new Date(time).toLocaleDateString(undefined, { dateStyle: "long" });
}

function host(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

function size(bytes: number): string {
  const [unit, divisor] = unitOf(bytes);
  return `${amount(bytes, divisor)} ${unit}`;
}

function unitOf(bytes: number): [string, number] {
  if (bytes >= 1e9) return ["GB", 1e9];
  if (bytes >= 1e6) return ["MB", 1e6];
  if (bytes >= 1e3) return ["KB", 1e3];
  return ["B", 1];
}

function amount(bytes: number, divisor: number): string {
  return divisor === 1 ? String(bytes) : (bytes / divisor).toFixed(1);
}

function duration(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)} min`;
  return `${Math.round(seconds / 3600)} h`;
}
