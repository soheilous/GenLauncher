// A small TGA reader: the game's map previews are .tga files, which browsers cannot show.
// Handles what maps use: true-colour (type 2) and run-length true-colour (type 10), 24 or 32 bits, any origin corner.

export interface Picture {
  width: number;
  height: number;
  /** RGBA, row by row from the top left. */
  rgba: Uint8ClampedArray<ArrayBuffer>;
}

export function decodeTga(bytes: Uint8Array): Picture {
  if (bytes.length < 18) throw new Error("not a TGA file");
  const idLength = bytes[0];
  const colorMapType = bytes[1];
  const type = bytes[2];
  const width = bytes[12] | (bytes[13] << 8);
  const height = bytes[14] | (bytes[15] << 8);
  const bits = bytes[16];
  const descriptor = bytes[17];
  if (colorMapType !== 0 || (type !== 2 && type !== 10) || (bits !== 24 && bits !== 32) || !width || !height) throw new Error("unsupported TGA");
  if (width * height > 4096 * 4096) throw new Error("TGA too large");

  const bytesPerPixel = bits / 8;
  let pos = 18 + idLength;
  const raw = new Uint8Array(width * height * bytesPerPixel);

  if (type === 2) {
    raw.set(bytes.subarray(pos, pos + raw.length));
  } else {
    let out = 0;
    while (out < raw.length && pos < bytes.length) {
      const head = bytes[pos++];
      const count = (head & 0x7f) + 1;
      if (head & 0x80) {
        // one pixel repeated
        for (let i = 0; i < count && out < raw.length; i++) {
          for (let b = 0; b < bytesPerPixel; b++) raw[out++] = bytes[pos + b];
        }
        pos += bytesPerPixel;
      } else {
        const n = Math.min(count * bytesPerPixel, raw.length - out);
        raw.set(bytes.subarray(pos, pos + n), out);
        out += n;
        pos += count * bytesPerPixel;
      }
    }
  }

  const topDown = (descriptor & 0x20) !== 0;
  const rightToLeft = (descriptor & 0x10) !== 0;
  const rgba = new Uint8ClampedArray(new ArrayBuffer(width * height * 4));
  let anyAlpha = false;
  if (bits === 32) for (let i = 3; i < raw.length; i += 4) if (raw[i] !== 0) { anyAlpha = true; break; }
  for (let y = 0; y < height; y++) {
    const srcY = topDown ? y : height - 1 - y;
    for (let x = 0; x < width; x++) {
      const srcX = rightToLeft ? width - 1 - x : x;
      const s = (srcY * width + srcX) * bytesPerPixel;
      const d = (y * width + x) * 4;
      rgba[d] = raw[s + 2];
      rgba[d + 1] = raw[s + 1];
      rgba[d + 2] = raw[s];
      // a 32 bit file whose alpha is all zero means "no alpha" in practice
      rgba[d + 3] = bits === 32 && anyAlpha ? raw[s + 3] : 255;
    }
  }
  return { width, height, rgba };
}

/** A data address of the picture as a PNG, for an <img>. Needs a browser (a canvas). */
export function tgaToDataUrl(bytes: Uint8Array, maxSide = 256): string {
  const pic = decodeTga(bytes);
  const full = document.createElement("canvas");
  full.width = pic.width;
  full.height = pic.height;
  full.getContext("2d")!.putImageData(new ImageData(pic.rgba, pic.width, pic.height), 0, 0);
  const scale = Math.min(1, maxSide / Math.max(pic.width, pic.height));
  if (scale === 1) return full.toDataURL("image/png");
  const small = document.createElement("canvas");
  small.width = Math.max(1, Math.round(pic.width * scale));
  small.height = Math.max(1, Math.round(pic.height * scale));
  small.getContext("2d")!.drawImage(full, 0, 0, small.width, small.height);
  return small.toDataURL("image/png");
}
