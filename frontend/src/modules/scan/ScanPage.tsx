import { Camera, CameraOff, ScanLine } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
import {
  type ScanTarget,
  parseScanPayload,
  scanTargetPath,
} from "../../shared/lib/qrPayload";
import { Button } from "../../shared/ui/Button";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { useInstanceIdentity } from "../labels/api";

/**
 * Where a scanned code leads.
 *
 * Resolution is entirely local: the payload carries the node and laboratory it
 * came from, which is matched first against this instance and then against the
 * federation trusts already loaded for the sidebar's laboratory switcher. That
 * is what lets a code printed by a partner laboratory open here — and what
 * makes an untrusted one fail closed instead of 404ing somewhere confusing.
 */
type Resolution =
  | { kind: "local"; target: ScanTarget }
  | { kind: "remote"; remoteLaboratoryId: string; remoteNodeId: string; target: ScanTarget }
  | { kind: "unknown-laboratory"; target: ScanTarget }
  | { kind: "unreadable" };

export function ScanPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const {
    federationTrusts,
    federationTrustsLoading,
    setSelectedLaboratoryId,
    setSelectedRemoteLaboratory,
  } = useLaboratorySelection();
  const identityQuery = useInstanceIdentity();

  const [manualValue, setManualValue] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  const localNodeId = identityQuery.data?.node_id;

  const resolve = useCallback(
    (value: string): Resolution => {
      const target = parseScanPayload(value);
      if (!target) {
        return { kind: "unreadable" };
      }
      if (localNodeId && target.nodeId === localNodeId) {
        return { kind: "local", target };
      }
      const trust = federationTrusts.find(
        (candidate) =>
          candidate.remote_node_id === target.nodeId &&
          candidate.remote_laboratory_id === target.laboratoryId,
      );
      if (trust) {
        return {
          kind: "remote",
          remoteLaboratoryId: trust.remote_laboratory_id,
          remoteNodeId: trust.remote_node_id,
          target,
        };
      }
      return { kind: "unknown-laboratory", target };
    },
    [federationTrusts, localNodeId],
  );

  const follow = useCallback(
    (value: string) => {
      const resolution = resolve(value);
      switch (resolution.kind) {
        case "local":
          // Switching scope first means the detail page's very first fetch
          // already targets the right laboratory.
          setSelectedLaboratoryId(resolution.target.laboratoryId);
          navigate(scanTargetPath(resolution.target));
          return true;
        case "remote":
          setSelectedRemoteLaboratory(
            resolution.remoteNodeId,
            resolution.remoteLaboratoryId,
          );
          navigate(scanTargetPath(resolution.target));
          return true;
        case "unknown-laboratory":
          setProblem(
            "这个二维码属于尚未建立联邦互信的实验室，无法查看。请联系管理员添加互信关系。",
          );
          return false;
        default:
          setProblem("无法识别这个二维码，它可能不是本系统生成的。");
          return false;
      }
    },
    [navigate, resolve, setSelectedLaboratoryId, setSelectedRemoteLaboratory],
  );

  // A code scanned with a phone camera lands here as a URL, so the parameters
  // are already in the address bar. Wait for the data resolution depends on
  // before deciding, or a federated code would be rejected as untrusted.
  const query = searchParams.toString();
  const ready = Boolean(localNodeId) && !federationTrustsLoading;
  useEffect(() => {
    if (!query || !ready) {
      return;
    }
    follow(query);
    // `follow` is stable for a given trust list; re-running on every render
    // would fight the navigation it performs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, ready]);

  return (
    <main className="page">
      <PageHeader kicker="工具" title="扫码" />

      <section className="panel">
        <div className="panel-body scan-panel">
          {problem ? <p className="field-error">{problem}</p> : null}

          <CameraScanner
            onDetected={(value) => {
              setProblem(null);
              return follow(value);
            }}
            disabled={!ready}
          />

          <form
            className="scan-manual"
            onSubmit={(event) => {
              event.preventDefault();
              setProblem(null);
              follow(manualValue);
            }}
          >
            <FormField
              htmlFor="scan-manual-input"
              label="手动输入"
              hint="也可以直接粘贴二维码里的链接。"
            >
              <input
                // The wrapping label also carries the hint text, so an explicit
                // name keeps the accessible name to just the field.
                aria-label="手动输入二维码内容"
                className="input"
                id="scan-manual-input"
                placeholder="粘贴二维码内容"
                value={manualValue}
                onChange={(event) => setManualValue(event.target.value)}
              />
            </FormField>
            <Button disabled={!manualValue.trim() || !ready} type="submit">
              <ScanLine size={16} />
              打开
            </Button>
          </form>
        </div>
      </section>
    </main>
  );
}

/** Raised when the page is not a secure context, so there is no camera API. */
class InsecureContextError extends Error {}

/**
 * Turns a `getUserMedia` failure into something the user can act on.
 *
 * The three causes need three different actions, and the old catch-all —
 * "确认页面通过 HTTPS 访问" — sent people chasing TLS when the real problem was
 * usually a denied permission.
 */
function describeCameraError(error: unknown) {
  if (error instanceof InsecureContextError) {
    return "当前页面不是安全上下文，浏览器不允许使用摄像头。请改用桌面端/安卓客户端，或让后端提供 HTTPS 访问，也可以直接手动输入。";
  }
  if (error instanceof DOMException) {
    switch (error.name) {
      case "NotAllowedError":
      case "SecurityError":
        return "摄像头权限被拒绝。请在系统设置或浏览器的权限设置里允许本应用使用摄像头后重试。";
      case "NotFoundError":
      case "OverconstrainedError":
        return "没有找到可用的摄像头。";
      case "NotReadableError":
        return "摄像头被其他应用占用，关掉之后再试。";
    }
  }
  return "无法启用摄像头，请改用手动输入。";
}

type CameraScannerProps = {
  disabled: boolean;
  /** Returns whether the value was accepted, so scanning can keep going. */
  onDetected: (value: string) => boolean;
};

/**
 * Live camera scanning.
 *
 * Uses the platform `BarcodeDetector` where it exists and falls back to a
 * WebAssembly build of the same API otherwise, so there is one code path rather
 * than two.
 *
 * The camera needs a secure context, which a browser on a plain-HTTP LAN
 * address is not — there the manual field is the only way in. The desktop and
 * mobile clients are unaffected: they serve the page from
 * `http://tauri.localhost`, and Chromium counts `.localhost` as trustworthy no
 * matter what the backend address is.
 */
function CameraScanner({ disabled, onDetected }: CameraScannerProps) {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const [scanning, setScanning] = useState(false);
  const [cameraError, setCameraError] = useState<string | null>(null);

  const stop = useCallback(() => {
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }
    setScanning(false);
  }, []);

  // Releasing the camera on unmount matters: the indicator light staying on
  // after navigating away reads as the app spying.
  useEffect(() => stop, [stop]);

  useEffect(() => {
    if (!scanning) {
      return;
    }

    let cancelled = false;
    let timer: number | undefined;

    async function run() {
      try {
        const { BarcodeDetector } = await import("barcode-detector/ponyfill");
        const detector = new BarcodeDetector({ formats: ["qr_code"] });

        // Outside a secure context the browser does not expose the API at all,
        // which would otherwise surface as an unhelpful TypeError.
        if (!navigator.mediaDevices?.getUserMedia) {
          throw new InsecureContextError();
        }
        const stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment" },
        });
        if (cancelled) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }
        streamRef.current = stream;

        const video = videoRef.current;
        if (!video) {
          return;
        }
        video.srcObject = stream;
        await video.play();

        const tick = async () => {
          if (cancelled || !videoRef.current) {
            return;
          }
          try {
            const codes = await detector.detect(videoRef.current);
            const value = codes[0]?.rawValue;
            if (value && onDetected(value)) {
              stop();
              return;
            }
          } catch {
            // A single failed frame is not worth surfacing; the next one will
            // very likely decode.
          }
          timer = window.setTimeout(tick, 250);
        };
        void tick();
      } catch (error) {
        if (cancelled) {
          return;
        }
        setCameraError(describeCameraError(error));
        setScanning(false);
      }
    }

    void run();

    return () => {
      cancelled = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    };
  }, [onDetected, scanning, stop]);

  return (
    <div className="scan-camera">
      <video className="scan-video" muted playsInline ref={videoRef} hidden={!scanning} />
      {cameraError ? <p className="field-error">{cameraError}</p> : null}
      {scanning ? (
        <Button variant="ghost" onClick={stop}>
          <CameraOff size={16} />
          停止扫描
        </Button>
      ) : (
        <Button
          disabled={disabled}
          onClick={() => {
            setCameraError(null);
            setScanning(true);
          }}
        >
          <Camera size={16} />
          使用摄像头扫码
        </Button>
      )}
    </div>
  );
}
