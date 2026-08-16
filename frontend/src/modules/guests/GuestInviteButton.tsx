import { Copy, RefreshCw, UserPlus } from "lucide-react";
import { useEffect, useState } from "react";
import { useAuth } from "../../app/auth-context";
import { toErrorMessage } from "../../shared/lib/errors";
import { Button } from "../../shared/ui/Button";
import { Dialog } from "../../shared/ui/Dialog";
import { useToast } from "../../shared/ui/Toast";
import { canCreateGuestRegistrationCode } from "../auth/permissions";
import { type RegistrationCode, useCreateGuestRegistrationCode } from "./api";

/**
 * Hands out a registration code so an outsider can create their own read-only
 * account in this laboratory.
 *
 * Renders nothing for anyone the API would refuse — server admins included,
 * since a code belongs to a laboratory and they belong to none.
 */
export function GuestInviteButton() {
  const { currentUser } = useAuth();
  const toast = useToast();
  const createCode = useCreateGuestRegistrationCode();
  const [open, setOpen] = useState(false);
  const [code, setCode] = useState<RegistrationCode | null>(null);

  if (!canCreateGuestRegistrationCode(currentUser)) {
    return null;
  }

  function generate() {
    createCode.mutate(undefined, {
      onError: (error) =>
        toast.error({ title: "生成注册码失败", description: toErrorMessage(error) }),
      onSuccess: (created) => setCode(created),
    });
  }

  return (
    <>
      <Button
        onClick={() => {
          // A code from an earlier visit has almost certainly expired, and
          // showing a dead one is worse than showing none.
          setCode(null);
          setOpen(true);
        }}
      >
        <UserPlus size={15} />
        邀请访客
      </Button>
      <Dialog
        description="访客凭注册码自助创建只读账号，账号归属当前实验室。"
        footer={
          <>
            <Button onClick={() => setOpen(false)}>关闭</Button>
            <Button disabled={createCode.isPending} variant="primary" onClick={generate}>
              {code ? <RefreshCw size={15} /> : <UserPlus size={15} />}
              {code ? "重新生成" : "生成注册码"}
            </Button>
          </>
        }
        open={open}
        title="邀请访客"
        onOpenChange={(next) => {
          setOpen(next);
          if (!next) {
            setCode(null);
          }
        }}
      >
        {code ? <IssuedCode code={code} /> : <InviteExplainer />}
      </Dialog>
    </>
  );
}

function InviteExplainer() {
  return (
    <ul className="guest-invite-notes">
      <li>注册码有效期 10 分钟，过期后需要重新生成。</li>
      <li>每个实验室同时只有一个有效的注册码，重新生成会让上一个立即失效。</li>
      <li>把注册码交给对方，让对方在登录页点「注册访客账号」填入。</li>
    </ul>
  );
}

function IssuedCode({ code }: { code: RegistrationCode }) {
  const toast = useToast();
  const remaining = useCountdown(code.expires_at);
  const expired = remaining <= 0;

  async function copy() {
    try {
      await navigator.clipboard.writeText(code.registration_code);
      toast.success({ title: "注册码已复制" });
    } catch {
      // Clipboard access can be refused; the code is on screen either way.
      toast.error({ title: "复制失败", description: "请手动选中注册码复制。" });
    }
  }

  return (
    <div className="guest-invite-issued">
      <div className="guest-invite-code">
        <code>{code.registration_code}</code>
        <Button aria-label="复制注册码" size="icon" variant="ghost" onClick={copy}>
          <Copy size={15} />
        </Button>
      </div>
      <p className={expired ? "field-error" : "guest-invite-expiry"}>
        {expired ? "注册码已过期，请重新生成。" : `剩余有效时间 ${formatRemaining(remaining)}`}
      </p>
      <InviteExplainer />
    </div>
  );
}

/** Milliseconds left before `expiresAt`, ticking once a second. */
function useCountdown(expiresAt: string) {
  const deadline = new Date(expiresAt).getTime();
  const [remaining, setRemaining] = useState(() => deadline - Date.now());

  useEffect(() => {
    setRemaining(deadline - Date.now());
    const timer = window.setInterval(() => setRemaining(deadline - Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [deadline]);

  return remaining;
}

function formatRemaining(milliseconds: number) {
  const totalSeconds = Math.max(0, Math.ceil(milliseconds / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
