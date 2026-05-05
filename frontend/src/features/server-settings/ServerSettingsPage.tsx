import { ApiOutlined, CheckCircleOutlined } from "@ant-design/icons";
import { useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Form, Input, Typography } from "antd";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  BackendConfigError,
  normalizeApiBaseUrl,
  useBackendConfig,
} from "../../shared/api/backendConfig";
import { EntryShell } from "../../shared/ui/EntryShell";
import { useTestBackendConnection } from "./api";

const { Paragraph, Text } = Typography;

export function ServerSettingsPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { apiBaseUrl, setApiBaseUrl } = useBackendConfig();
  const [draftUrl, setDraftUrl] = useState(apiBaseUrl);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const testConnection = useTestBackendConnection();

  function saveDraft() {
    try {
      const normalized = setApiBaseUrl(draftUrl);
      queryClient.clear();
      setDraftUrl(normalized);
      setError(null);
      navigate("/", { replace: true });
    } catch (caught) {
      setError(toMessage(caught));
      setMessage(null);
    }
  }

  function testDraft() {
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
        setError(`${toMessage(caught)} 请确认地址、网络、CORS 和后端服务状态。`);
        setMessage(null);
      },
    });
  }

  return (
    <EntryShell
      title="后端服务器设置"
      titleId="server-settings-title"
      description="先确认当前客户端连接的后端 API，再逐步进入登录、概览和库存工作流。"
      cardTitle="连接设置"
      cardIcon={<ApiOutlined aria-hidden="true" />}
      meta={
        <div className="entry-server-line">
          <Text type="secondary">当前服务器</Text>
          <Text code className="entry-server-code">
            {apiBaseUrl}
          </Text>
        </div>
      }
    >
      <Form layout="vertical" onFinish={saveDraft} size="large">
        <Form.Item label="后端 API 地址" htmlFor="backend-api-url" required>
          <Input
            id="backend-api-url"
            value={draftUrl}
            onChange={(event) => setDraftUrl(event.target.value)}
            placeholder="http://127.0.0.1:8000/api/v1"
          />
        </Form.Item>
        <Paragraph type="secondary" className="entry-form-help">
          可以填写服务器根地址，系统会自动补齐 /api/v1。
        </Paragraph>

        {message ? (
          <Alert
            showIcon
            type="success"
            icon={<CheckCircleOutlined />}
            title={message}
          />
        ) : null}
        {error ? <Alert showIcon type="error" title={error} /> : null}

        <div className="entry-form-actions">
          <Button
            type="default"
            size="large"
            onClick={testDraft}
            loading={testConnection.isPending}
          >
            测试连接
          </Button>
          <Button aria-label="继续" type="primary" size="large" htmlType="submit">
            继续
          </Button>
        </div>
      </Form>
    </EntryShell>
  );
}

function toMessage(caught: unknown) {
  if (caught instanceof BackendConfigError || caught instanceof Error) {
    return caught.message;
  }
  return "操作失败。";
}
