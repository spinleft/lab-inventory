import * as Dialog from "@radix-ui/react-dialog";
import { ServerCog, X } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  BackendConfigError,
  normalizeApiBaseUrl,
  useBackendConfig,
} from "../../shared/api/backendConfig";
import { Button } from "../../shared/ui/Button";
import { IconButton } from "../../shared/ui/IconButton";
import { TextInput } from "../../shared/ui/TextInput";
import { useTestBackendConnection } from "./api";

type ServerSettingsDialogProps = {
  trigger: ReactNode;
};

export function ServerSettingsDialog({ trigger }: ServerSettingsDialogProps) {
  const queryClient = useQueryClient();
  const { apiBaseUrl, defaultApiBaseUrl, resetApiBaseUrl, setApiBaseUrl } =
    useBackendConfig();
  const [open, setOpen] = useState(false);
  const [draftUrl, setDraftUrl] = useState(apiBaseUrl);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const testConnection = useTestBackendConnection();

  function handleOpenChange(nextOpen: boolean) {
    setOpen(nextOpen);
    if (nextOpen) {
      setDraftUrl(apiBaseUrl);
      setMessage(null);
      setError(null);
      testConnection.reset();
    }
  }

  function save() {
    try {
      setApiBaseUrl(draftUrl);
      queryClient.clear();
      setOpen(false);
    } catch (caught) {
      setError(toMessage(caught));
      setMessage(null);
    }
  }

  function reset() {
    resetApiBaseUrl();
    queryClient.clear();
    setDraftUrl(defaultApiBaseUrl);
    setMessage("已恢复默认服务器地址。");
    setError(null);
  }

  function test() {
    let normalized: string;
    try {
      normalized = normalizeApiBaseUrl(draftUrl);
      setDraftUrl(normalized);
    } catch (caught) {
      setError(toMessage(caught));
      setMessage(null);
      return;
    }

    testConnection.mutate(normalized, {
      onSuccess: () => {
        setMessage("连接正常。");
        setError(null);
      },
      onError: (caught) => {
        setError(
          `${toMessage(caught)} 请确认地址、网络、CORS 和后端服务状态。`,
        );
        setMessage(null);
      },
    });
  }

  return (
    <Dialog.Root open={open} onOpenChange={handleOpenChange}>
      <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content">
          <div className="dialog-header">
            <div className="cluster">
              <ServerCog aria-hidden="true" size={20} />
              <Dialog.Title className="dialog-title">服务器设置</Dialog.Title>
            </div>
            <Dialog.Close asChild>
              <IconButton label="关闭">
                <X size={18} />
              </IconButton>
            </Dialog.Close>
          </div>
          <Dialog.Description className="dialog-description">
            设置当前客户端连接的后端 API 地址。
          </Dialog.Description>

          <div className="stack dialog-body">
            <label className="form-row">
              <span className="label">后端 API 地址</span>
              <TextInput
                value={draftUrl}
                onChange={(event) => setDraftUrl(event.target.value)}
                placeholder="http://127.0.0.1:8000/api/v1"
              />
            </label>
            <p className="muted small">
              可以填写服务器根地址，系统会自动补齐 /api/v1。
            </p>
            {message ? <div className="notice">{message}</div> : null}
            {error ? <div className="alert">{error}</div> : null}
            <div className="dialog-actions">
              <Button
                type="button"
                variant="secondary"
                onClick={test}
                disabled={testConnection.isPending}
              >
                {testConnection.isPending ? "测试中..." : "测试连接"}
              </Button>
              <Button type="button" variant="ghost" onClick={reset}>
                恢复默认
              </Button>
              <Button type="button" onClick={save}>
                保存
              </Button>
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function toMessage(caught: unknown) {
  if (caught instanceof BackendConfigError || caught instanceof Error) {
    return caught.message;
  }
  return "操作失败。";
}
