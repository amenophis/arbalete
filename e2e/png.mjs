// Writes small PNG images of one color, as test favicons.

import { crc32, deflateSync } from "node:zlib";

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

// A `size` × `size` square of the opaque color [red, green, blue].
export function png(size, [red, green, blue]) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8; // bits per channel
  header[9] = 6; // RGBA
  const row = Buffer.from([
    0,
    ...Array.from({ length: size }, () => [red, green, blue, 255]).flat(),
  ]);
  const pixels = Buffer.concat(Array.from({ length: size }, () => row));
  return Buffer.concat([
    SIGNATURE,
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(pixels)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}
