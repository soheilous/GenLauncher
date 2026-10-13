// Client-side helpers for user-chosen mod covers.
// The Rust core will later store the processed file on disk instead of localStorage.

const MAX_W = 1280;

function loadImage(file: Blob): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => { URL.revokeObjectURL(url); resolve(img); };
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error("Not a readable image")); };
    img.src = url;
  });
}

/** Dominant hue (0-359): circular mean of pixel hues weighted by saturation and mid lightness. */
function dominantHue(img: HTMLImageElement): number {
  const w = 48, h = 27;
  const c = document.createElement("canvas");
  c.width = w; c.height = h;
  const ctx = c.getContext("2d")!;
  ctx.drawImage(img, 0, 0, w, h);
  const data = ctx.getImageData(0, 0, w, h).data;
  let x = 0, y = 0;
  for (let i = 0; i < data.length; i += 4) {
    const r = data[i] / 255, g = data[i + 1] / 255, b = data[i + 2] / 255;
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    const l = (max + min) / 2, d = max - min;
    if (d < 0.08) continue; // grey
    const s = d / (1 - Math.abs(2 * l - 1));
    let hue = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    hue = (hue * 60 + 360) % 360;
    const weight = s * (1 - Math.abs(2 * l - 1));
    x += Math.cos((hue * Math.PI) / 180) * weight;
    y += Math.sin((hue * Math.PI) / 180) * weight;
  }
  if (x === 0 && y === 0) return 262;
  return Math.round(((Math.atan2(y, x) * 180) / Math.PI + 360) % 360);
}

export interface ProcessedCover { dataUrl: string; hue: number }

/** Downscale to <=1280px wide JPEG (keeps localStorage small) and extract the accent hue. */
export async function processCover(file: Blob): Promise<ProcessedCover> {
  if (!file.type.startsWith("image/")) throw new Error("Please choose an image file");
  const img = await loadImage(file);
  const scale = Math.min(1, MAX_W / img.naturalWidth);
  const c = document.createElement("canvas");
  c.width = Math.round(img.naturalWidth * scale);
  c.height = Math.round(img.naturalHeight * scale);
  c.getContext("2d")!.drawImage(img, 0, 0, c.width, c.height);
  return { dataUrl: c.toDataURL("image/jpeg", 0.82), hue: dominantHue(img) };
}
