import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  BackendConfigError,
  normalizeApiBaseUrl,
  useBackendConfig,
} from "../../shared/api/backendConfig";
import { Button } from "../../shared/ui/Button";
import { TextInput } from "../../shared/ui/TextInput";
import { useTestBackendConnection } from "./api";

type ServerSettingsFormProps = {
  actionsClassName?: string;
  onSaved?: (apiBaseUrl: string) => void | Promise<void>;
  saveLabel?: string;
  savingLabel?: string;
};

export function ServerSettingsForm({
  actionsClassName = "dialog-actions",
  onSaved,
  saveLabel = "保存",
  savingLabel = "保存中...",
}: ServerSettingsFormProps) {
  const queryClient = useQueryClient();
  const { apiBaseUrl, defaultApiBaseUrl, resetApiBaseUrl, setApiBaseUrl } =
    useBackendConfig();
  const [draftUrl, setDraftUrl] = useState(apiBaseUrl);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const testConnection = useTestBackendConnection();

  async function save() {
    try {
      setIsSaving(true);
      const normalized = setApiBaseUrl(draftUrl);
      queryClient.clear();
      setDraftUrl(normalized);
      await onSaved?.(normalized);
    } catch (caught) {
      setError(toMessage(caught));
      setMessage(null);
    } finally {
      setIsSaving(false);
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
    <div className="stack">
      <label className="form-row">
        <span className="label">后端 API 地址</span>
        <TextInput
          value={draftUrl}
          onChange={(event) => setDraftUrl(event.target.value)}
          placeholder="http://127.0.0.1:8000/api/v1"
        />
      </label>
      <p className="muted small">可以填写服务器根地址，系统会自动补齐 /api/v1。</p>
      {message ? <div className="notice">{message}</div> : null}
      {error ? <div className="alert">{error}</div> : null}
      <div className={actionsClassName}>
        <Button
          type="button"
          variant="secondary"
          onClick={test}
          disabled={testConnection.isPending || isSaving}
        >
          {testConnection.isPending ? "测试中..." : "测试连接"}
        </Button>
        <Button type="button" variant="ghost" onClick={reset} disabled={isSaving}>
          恢复默认
        </Button>
        <Button type="button" onClick={save} disabled={isSaving}>
          {isSaving ? savingLabel : saveLabel}
        </Button>
      </div>
    </div>
  );
}

function toMessage(caught: unknown) {
  if (caught instanceof BackendConfigError || caught instanceof Error) {
    return caught.message;
  }
  return "操作失败。";
}
