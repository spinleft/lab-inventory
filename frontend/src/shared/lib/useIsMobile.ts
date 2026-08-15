import { useEffect, useState } from "react";

/**
 * Viewport width at which the app switches between its two layouts.
 *
 * Below this the shell is a phone app — bottom tabs, sheets, card lists; at or
 * above it, the desktop sidebar. Tablets deliberately land on the desktop side:
 * they have the width for it, and the density is what makes the tables usable.
 *
 * Kept in sync with the `@media` blocks in `styles/mobile.css`.
 */
export const MOBILE_BREAKPOINT = 768;

const QUERY = `(max-width: ${MOBILE_BREAKPOINT - 1}px)`;

export function useIsMobile() {
  const [isMobile, setIsMobile] = useState(() => matchMedia(QUERY).matches);

  useEffect(() => {
    const media = matchMedia(QUERY);
    const update = () => setIsMobile(media.matches);
    // Rotating the device fires this, and so does dragging a desktop window
    // across the breakpoint.
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  return isMobile;
}
