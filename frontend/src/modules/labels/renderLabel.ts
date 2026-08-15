import QRCode from "qrcode";
import type { LabelLayout } from "./api";

/**
 * Drawing a label onto a canvas at printer resolution.
 *
 * The canvas is sized in printer dots rather than CSS pixels, and is never
 * scaled afterwards: resampling would soften the QR code's module edges, which
 * is the difference between a code that scans across a bench and one that does
 * not. Anything shown on screen is a scaled *view* of this canvas, not the
 * canvas itself.
 */

/** Default length for continuous stock, in dots (roughly 25mm at 300 dpi). */
const DEFAULT_CONTINUOUS_LENGTH_DOTS = 300;

/** Blank dots kept clear on every side. */
const PADDING_DOTS = 16;

/** Gap between the QR code and the text column. */
const GUTTER_DOTS = 16;

export type LabelContent = {
  /** Small monospace line under the title, e.g. a serial number. */
  code?: string | null;
  /** Secondary line, e.g. the model. */
  subtitle?: string | null;
  /** The QR payload. */
  payload: string;
  /** Primary line, typically the asset name. Chinese. */
  title: string;
};

/** The dot dimensions a label of this stock should be rendered at. */
export function labelSize(layout: LabelLayout) {
  const heightDots =
    layout.printable_length_dots > 0
      ? layout.printable_length_dots
      : Math.min(
          Math.max(DEFAULT_CONTINUOUS_LENGTH_DOTS, layout.min_length_dots),
          layout.max_length_dots,
        );

  return { heightDots, widthDots: layout.printable_width_dots };
}

/**
 * Draws a label and returns the canvas, sized in printer dots.
 *
 * The QR code is rendered by the same encoder in both the preview and the print
 * path, so what is shown is what is printed.
 */
export async function renderLabel(
  layout: LabelLayout,
  content: LabelContent,
): Promise<HTMLCanvasElement> {
  const { heightDots, widthDots } = labelSize(layout);

  const canvas = document.createElement("canvas");
  canvas.width = widthDots;
  canvas.height = heightDots;

  const context = canvas.getContext("2d");
  if (!context) {
    throw new Error("无法创建标签画布。");
  }

  // Paper first: the packer treats anything light as blank, but being explicit
  // keeps the preview honest too.
  context.fillStyle = "#ffffff";
  context.fillRect(0, 0, widthDots, heightDots);

  const qrSize = Math.min(heightDots - PADDING_DOTS * 2, Math.floor(widthDots * 0.45));
  const qrCanvas = await renderQrCode(content.payload, qrSize);
  const qrTop = Math.floor((heightDots - qrSize) / 2);
  context.drawImage(qrCanvas, PADDING_DOTS, qrTop, qrSize, qrSize);

  drawText(context, content, {
    left: PADDING_DOTS + qrSize + GUTTER_DOTS,
    top: PADDING_DOTS,
    width: widthDots - (PADDING_DOTS * 2 + qrSize + GUTTER_DOTS),
    height: heightDots - PADDING_DOTS * 2,
  });

  return canvas;
}

/**
 * Renders the QR payload to its own canvas at an exact module size.
 *
 * `margin: 0` because the label already reserves its own quiet zone through
 * padding, and letting the encoder add another would shrink the modules.
 */
async function renderQrCode(payload: string, sizeDots: number) {
  const canvas = document.createElement("canvas");
  await QRCode.toCanvas(canvas, payload, {
    color: { dark: "#000000ff", light: "#ffffffff" },
    errorCorrectionLevel: "M",
    margin: 1,
    width: sizeDots,
  });
  return canvas;
}

type TextBox = {
  height: number;
  left: number;
  top: number;
  width: number;
};

function drawText(
  context: CanvasRenderingContext2D,
  content: LabelContent,
  box: TextBox,
) {
  context.fillStyle = "#000000";
  context.textBaseline = "top";

  const lines: { font: string; lineHeight: number; text: string }[] = [];
  const titleSize = Math.floor(box.height * 0.22);
  lines.push({
    font: `bold ${titleSize}px sans-serif`,
    lineHeight: Math.floor(titleSize * 1.25),
    text: content.title,
  });

  if (content.subtitle) {
    const size = Math.floor(titleSize * 0.72);
    lines.push({
      font: `${size}px sans-serif`,
      lineHeight: Math.floor(size * 1.3),
      text: content.subtitle,
    });
  }

  if (content.code) {
    const size = Math.floor(titleSize * 0.68);
    lines.push({
      font: `${size}px monospace`,
      lineHeight: Math.floor(size * 1.3),
      text: content.code,
    });
  }

  let y = box.top;
  for (const line of lines) {
    if (y + line.lineHeight > box.top + box.height) {
      break;
    }
    context.font = line.font;
    // Wrapping by character rather than by word: Chinese has no spaces to
    // break on, and a truncated asset name is what makes a label useless.
    const wrapped = wrapText(context, line.text, box.width, 2);
    for (const text of wrapped) {
      if (y + line.lineHeight > box.top + box.height) {
        break;
      }
      context.fillText(text, box.left, y);
      y += line.lineHeight;
    }
  }
}

/**
 * Breaks text to fit a width, character by character, capped at `maxLines`.
 *
 * The final line is ellipsised rather than silently cut so it is obvious when a
 * name did not fit.
 */
export function wrapText(
  context: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
  maxLines: number,
): string[] {
  const lines: string[] = [];
  let current = "";

  for (const character of text) {
    const candidate = current + character;
    if (context.measureText(candidate).width <= maxWidth || !current) {
      current = candidate;
      continue;
    }

    lines.push(current);
    current = character;

    if (lines.length === maxLines) {
      break;
    }
  }

  if (lines.length < maxLines && current) {
    lines.push(current);
    return lines;
  }

  if (lines.length === maxLines && current) {
    // What is left did not fit; mark the last line as truncated.
    let last = lines[maxLines - 1];
    while (last && context.measureText(`${last}…`).width > maxWidth) {
      last = last.slice(0, -1);
    }
    lines[maxLines - 1] = `${last}…`;
  }

  return lines;
}
