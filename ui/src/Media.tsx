import { send } from "./bridge";
import type { Download } from "./Downloads";
import { matches } from "./search";

// The extensions of the images and videos the app serves to the Media section (media_type in
// app/src/downloads.rs).
const IMAGES = ["png", "jpg", "jpeg", "gif", "webp", "avif"];
const VIDEOS = ["mp4", "m4v", "webm", "mov"];

function kind(name: string): "image" | "video" | null {
  const extension = name.split(".").pop()?.toLowerCase() ?? "";
  if (!name.includes(".")) return null;
  if (IMAGES.includes(extension)) return "image";
  return VIDEOS.includes(extension) ? "video" : null;
}

// The images and videos downloaded and still there, the most recent first, as a grid of their
// previews under the library's search field, like Arc's Media; a click opens one.
export function MediaList({ downloads, query }: { downloads: Download[]; query: string }) {
  const media = downloads
    .filter((download) => download.state === "complete" && !download.missing && kind(download.name))
    .reverse();
  if (media.length === 0) return <p className="library-empty">No media yet</p>;
  const found = media.filter((download) => matches(query, download.name, download.url));
  if (found.length === 0) return <p className="library-empty">No media matches</p>;
  return (
    <ul className="media-list">
      {found.map((download) => (
        <li key={download.id}>
          <button
            type="button"
            className="media-tile"
            title={download.name}
            onClick={() => send(`download/open/${download.id}`)}
          >
            {kind(download.name) === "video" ? (
              <video src={`/media/${download.id}`} muted preload="metadata" />
            ) : (
              <img src={`/media/${download.id}`} alt="" loading="lazy" />
            )}
            <span className="title">{download.name}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
