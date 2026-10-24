import { useEffect, useState } from "react";

// The colors found by favicon url, null for none: an icon is read once.
const colors = new Map<string, string | null>();

// The dominant color of the favicon at `url`, or null while it loads, when it has no color of its
// own (gray, black or white) or when its server does not let pages read its pixels.
export function useFaviconColor(url: string | null): string | null {
  const [color, setColor] = useState<string | null>(null);
  useEffect(() => {
    setColor(url ? (colors.get(url) ?? null) : null);
    if (!url || colors.has(url)) return;
    let current = true;
    const image = new Image();
    image.crossOrigin = "anonymous";
    const found = (color: string | null) => {
      colors.set(url, color);
      if (current) setColor(color);
    };
    image.onload = () => found(readColor(image));
    image.onerror = () => found(null);
    image.src = url;
    return () => {
      current = false;
    };
  }, [url]);
  return color;
}

function readColor(image: HTMLImageElement): string | null {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 16;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(image, 0, 0, 16, 16);
  try {
    return dominantColor(context.getImageData(0, 0, 16, 16).data);
  } catch {
    // An image from a server that does not allow it taints the canvas.
    return null;
  }
}

// The average of the most common colors among RGBA pixels, grouped by close hues, leaving out
// transparent and gray pixels; null when every pixel is one of them.
export function dominantColor(pixels: ArrayLike<number>): string | null {
  const groups = new Map<number, number[]>();
  let best: number[] | undefined;
  for (let i = 0; i + 3 < pixels.length; i += 4) {
    const [r, g, b, a] = [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]];
    if (a < 128 || Math.max(r, g, b) - Math.min(r, g, b) < 32) continue;
    const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
    const group = groups.get(key) ?? [0, 0, 0, 0];
    group[0] += r;
    group[1] += g;
    group[2] += b;
    group[3] += 1;
    groups.set(key, group);
    if (!best || group[3] > best[3]) best = group;
  }
  if (!best) return null;
  const count = best[3];
  return `#${best
    .slice(0, 3)
    .map((sum) =>
      Math.round(sum / count)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}
