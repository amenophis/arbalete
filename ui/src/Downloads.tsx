import { useState } from "react";
import { send, useAppEvent } from "./bridge";
import { Icon } from "./Icon";

export type Download = {
  id: number;
  name: string;
  received: number;
  // 0 while the size is unknown.
  total: number;
  // Bytes per second.
  speed: number;
  state: "inProgress" | "complete" | "canceled" | "failed";
};

// The session's downloads at the bottom of the sidebar, as the app sends them
// (send_downloads in app/src/shell.rs): a finished one opens on a click, and shows in the Finder
// from its button; the × cancels one in progress, or takes another off the list.
export function Downloads() {
  const [downloads, setDownloads] = useState<Download[]>([]);
  useAppEvent<Download[]>("arbalete:downloads", setDownloads);
  if (downloads.length === 0) return null;
  return (
    <ul className="downloads" aria-label="Downloads">
      {downloads.map((download) => {
        const complete = download.state === "complete";
        const running = download.state === "inProgress";
        return (
          <li key={download.id} className="tab-row download">
            <button
              type="button"
              className="tab"
              disabled={!complete}
              title={download.name}
              onClick={() => send(`download/open/${download.id}`)}
            >
              <span className="favicon">
                <Icon d="M8 2.5v8M4.5 7 8 10.5 11.5 7M3 13.5h10" />
              </span>
              <span className="download-text">
                <span className="title">{download.name}</span>
                <span className="download-detail">{detail(download)}</span>
              </span>
              {running && download.total > 0 && (
                <progress value={download.received} max={download.total} aria-label="Progress" />
              )}
            </button>
            {complete && (
              <button
                type="button"
                className="close reveal"
                aria-label="Show in Finder"
                onClick={() => send(`download/show/${download.id}`)}
              >
                <Icon
                  d="M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3 13.5 13.5"
                  size={14}
                />
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
      })}
    </ul>
  );
}

// "12.3 / 45.6 MB · 8 s left" while downloading, then what became of it.
export function detail(download: Download): string {
  switch (download.state) {
    case "complete":
      return size(download.received);
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
