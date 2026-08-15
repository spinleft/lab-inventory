import { describe, expect, it } from "vitest";
import { packPixels, rowBytes, toBase64 } from "./bitmap";

/** Builds RGBA pixel data from a picture drawn with `#` for black. */
function pixelsFrom(rows: string[]): Uint8ClampedArray {
  const height = rows.length;
  const width = rows[0].length;
  const data = new Uint8ClampedArray(width * height * 4);

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const value = rows[y][x] === "#" ? 0 : 255;
      const offset = (y * width + x) * 4;
      data[offset] = value;
      data[offset + 1] = value;
      data[offset + 2] = value;
      data[offset + 3] = 255;
    }
  }

  return data;
}

describe("rowBytes", () => {
  it("rounds up to whole bytes", () => {
    expect(rowBytes(8)).toBe(1);
    expect(rowBytes(9)).toBe(2);
    // The two label widths that actually matter.
    expect(rowBytes(696)).toBe(87);
    expect(rowBytes(306)).toBe(39);
  });
});

describe("packPixels", () => {
  it("packs the leftmost dot into the high bit of the first byte", () => {
    const packed = packPixels(pixelsFrom(["#0000000"]), 8, 1);
    expect(packed).toEqual(new Uint8Array([0x80]));
  });

  it("packs bits MSB-first across a byte", () => {
    const packed = packPixels(pixelsFrom(["#0#0000#"]), 8, 1);
    expect(packed).toEqual(new Uint8Array([0b1010_0001]));
  });

  it("treats a set bit as a black dot, not a white one", () => {
    // All-white input must pack to zeros; getting this backwards would print
    // solid black labels.
    const packed = packPixels(pixelsFrom(["00000000", "00000000"]), 8, 2);
    expect(packed).toEqual(new Uint8Array([0x00, 0x00]));

    const inverted = packPixels(pixelsFrom(["########"]), 8, 1);
    expect(inverted).toEqual(new Uint8Array([0xff]));
  });

  it("pads each row to a whole byte without bleeding into the next", () => {
    // 9 dots wide: the second byte holds one dot and 7 bits of padding.
    const packed = packPixels(pixelsFrom(["00000000#", "#00000000"]), 9, 2);
    expect(packed).toEqual(new Uint8Array([0x00, 0x80, 0x80, 0x00]));
  });

  it("keeps rows aligned to the stride", () => {
    const rows = ["#0000000", "0#000000", "00#00000"];
    const packed = packPixels(pixelsFrom(rows), 8, 3);
    expect(packed).toEqual(new Uint8Array([0x80, 0x40, 0x20]));
  });

  it("composites transparency over white so a blank canvas prints blank", () => {
    // Fully transparent black pixels: the canvas default before anything is
    // drawn. These must read as paper, not ink.
    const data = new Uint8ClampedArray(8 * 4);
    const packed = packPixels(data, 8, 1);
    expect(packed).toEqual(new Uint8Array([0x00]));
  });

  it("thresholds mid greys by perceived luminance", () => {
    const data = new Uint8ClampedArray(2 * 4);
    // Dark grey -> ink.
    data.set([40, 40, 40, 255], 0);
    // Light grey -> paper.
    data.set([200, 200, 200, 255], 4);
    const packed = packPixels(data, 2, 1);
    expect(packed).toEqual(new Uint8Array([0b1000_0000]));
  });

  it("produces the exact byte count a label of a given size needs", () => {
    const width = 696;
    const height = 271;
    const data = new Uint8ClampedArray(width * height * 4);
    expect(packPixels(data, width, height).length).toBe(87 * 271);
  });
});

describe("toBase64", () => {
  it("round-trips through the browser decoder", () => {
    const bytes = new Uint8Array([0x00, 0x80, 0xff, 0x7f, 0x01]);
    const decoded = Uint8Array.from(atob(toBase64(bytes)), (c) =>
      c.charCodeAt(0),
    );
    expect(decoded).toEqual(bytes);
  });

  it("handles a payload larger than one chunk", () => {
    const bytes = new Uint8Array(87 * 271).map((_, index) => index % 256);
    const decoded = Uint8Array.from(atob(toBase64(bytes)), (c) =>
      c.charCodeAt(0),
    );
    expect(decoded.length).toBe(bytes.length);
    expect(decoded).toEqual(bytes);
  });
});
