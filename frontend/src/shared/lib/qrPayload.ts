/**
 * The contract printed on every label and read back by every scan.
 *
 * A payload is a plain URL so that it works two ways: a person can open it in a
 * phone browser and land on the instance that printed it, while the app can
 * ignore the host entirely and resolve the code from the query parameters. That
 * second path is what lets a federated user scan another laboratory's label —
 * the node id is matched against their own federation trusts, not against the
 * URL they scanned.
 *
 * Both sides of the feature go through this module, so the format cannot drift
 * between what is printed and what is understood.
 */

export const SCAN_PAYLOAD_VERSION = "1";

export type ScanTargetType = "asset" | "item";

export type ScanTarget = {
  laboratoryId: string;
  nodeId: string;
  resourceId: string;
  type: ScanTargetType;
};

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isUuid(value: string | null): value is string {
  return typeof value === "string" && UUID_PATTERN.test(value);
}

function isScanTargetType(value: string | null): value is ScanTargetType {
  return value === "asset" || value === "item";
}

/**
 * Builds the URL a label's QR code encodes.
 *
 * `webOrigin` is the instance's public web origin, which only matters for the
 * human-readable path; scanning resolves from the parameters.
 */
export function buildScanPayload(webOrigin: string, target: ScanTarget) {
  const parameters = new URLSearchParams({
    v: SCAN_PAYLOAD_VERSION,
    n: target.nodeId,
    l: target.laboratoryId,
    t: target.type,
    i: target.resourceId,
  });
  return `${webOrigin.replace(/\/+$/, "")}/scan?${parameters.toString()}`;
}

/**
 * Reads a scanned string back into a target, or returns `null` if it is not one
 * of ours.
 *
 * Accepts both a full payload URL and a bare query string, so a scanner that
 * hands back only part of the code still resolves.
 */
export function parseScanPayload(value: string): ScanTarget | null {
  const parameters = readParameters(value.trim());
  if (!parameters) {
    return null;
  }

  // Unknown versions are refused rather than guessed at: a future format may
  // reuse these keys with different meanings.
  if (parameters.get("v") !== SCAN_PAYLOAD_VERSION) {
    return null;
  }

  const nodeId = parameters.get("n");
  const laboratoryId = parameters.get("l");
  const resourceId = parameters.get("i");
  const type = parameters.get("t");

  if (!isUuid(nodeId) || !isUuid(laboratoryId) || !isUuid(resourceId)) {
    return null;
  }
  if (!isScanTargetType(type)) {
    return null;
  }

  return { laboratoryId, nodeId, resourceId, type };
}

function readParameters(value: string): URLSearchParams | null {
  if (!value) {
    return null;
  }

  try {
    // A relative URL still parses against a base, which covers payloads that
    // arrive as "/scan?..." rather than as a full URL.
    const parameters = new URL(value, "http://scan.invalid").searchParams;
    if (parameters.has("v")) {
      return parameters;
    }
    // A bare query string parses as a *path* rather than a query, leaving no
    // parameters behind, so fall through and read it directly.
  } catch {
    // Not a URL at all; same fallback applies.
  }

  const query = value.startsWith("?") ? value.slice(1) : value;
  if (!query.includes("=")) {
    return null;
  }
  return new URLSearchParams(query);
}

/** The in-app route a scanned target resolves to. */
export function scanTargetPath(target: ScanTarget) {
  return target.type === "asset"
    ? `/assets/${target.resourceId}`
    : `/inventory/${target.resourceId}`;
}
