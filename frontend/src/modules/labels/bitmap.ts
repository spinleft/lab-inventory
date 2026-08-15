/**
 * Turning a rendered canvas into the 1-bit bitmap the printer wants.
 *
 * The browser does the layout because label text is Chinese and the system
 * already has fonts for it; the server only wraps the result in raster
 * commands. That split means this module owns the one thing both sides have to
 * agree on: how the bits are packed.
 *
 * Packing matches the print head — rows are MSB-first, and **a set bit is a
 * black dot**, which is the opposite of the canvas convention where a high
 * luminance value is white.
 */

/** Luminance at or below which a pixel is printed. */
const BLACK_THRESHOLD = 127;

export type PackedBitmap = {
  bitmapBase64: string;
  heightDots: number;
  widthDots: number;
};

/** Bytes one row of `widthDots` pixels occupies. */
export function rowBytes(widthDots: number) {
  return Math.ceil(widthDots / 8);
}

/**
 * Packs RGBA pixel data into one bit per dot.
 *
 * Pixels are judged on perceived luminance, weighted against white so that a
 * transparent canvas reads as blank paper rather than as solid black.
 */
export function packPixels(
  pixels: Uint8ClampedArray,
  widthDots: number,
  heightDots: number,
): Uint8Array {
  const stride = rowBytes(widthDots);
  const packed = new Uint8Array(stride * heightDots);

  for (let y = 0; y < heightDots; y += 1) {
    for (let x = 0; x < widthDots; x += 1) {
      const offset = (y * widthDots + x) * 4;
      const alpha = pixels[offset + 3] / 255;
      // Composite over white before judging, so partial transparency lightens
      // rather than darkens.
      const luminance =
        (0.299 * pixels[offset] +
          0.587 * pixels[offset + 1] +
          0.114 * pixels[offset + 2]) *
          alpha +
        255 * (1 - alpha);

      if (luminance <= BLACK_THRESHOLD) {
        packed[y * stride + (x >> 3)] |= 0x80 >> (x & 7);
      }
    }
  }

  return packed;
}

export function toBase64(bytes: Uint8Array) {
  let binary = "";
  // Chunked so a full-length label does not blow the argument limit.
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(
      ...bytes.subarray(index, index + chunkSize),
    );
  }
  return btoa(binary);
}

/**
 * Reads a canvas back as a packed bitmap ready to POST.
 *
 * The canvas must already be sized in printer dots — scaling here would blur
 * the QR code's module edges, which is exactly what makes a code unreadable.
 */
export function packCanvas(canvas: HTMLCanvasElement): PackedBitmap {
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) {
    throw new Error("无法读取标签画布内容。");
  }

  const widthDots = canvas.width;
  const heightDots = canvas.height;
  const { data } = context.getImageData(0, 0, widthDots, heightDots);

  return {
    bitmapBase64: toBase64(packPixels(data, widthDots, heightDots)),
    heightDots,
    widthDots,
  };
}
