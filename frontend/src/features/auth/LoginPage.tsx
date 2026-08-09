import {
  LockOutlined,
  LoginOutlined,
  SettingOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Form, Input, Typography } from "antd";
import { Link, useNavigate } from "react-router-dom";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { EntryShell } from "../../shared/ui/EntryShell";
import { useLogin } from "./api";

const { Text } = Typography;

type LoginFormValues = {
  password: string;
  username: string;
};

export function LoginPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { apiBaseUrl } = useBackendConfig();
  const login = useLogin();

  function submitLogin(values: LoginFormValues) {
    login.mutate(
      {
        password: values.password,
        username: values.username.trim(),
      },
      {
        onSuccess: () => {
          queryClient.invalidateQueries({ queryKey: ["auth", "me"] });
          navigate("/dashboard", { replace: true });
        },
      },
    );
  }

  return (
    <EntryShell
      title="登录"
      titleId="login-title"
      description="使用后端账号进入实验室库存管理系统。"
      cardTitle="账号登录"
      cardIcon={<LoginOutlined aria-hidden="true" />}
    >
      <Form<LoginFormValues>
        layout="vertical"
        onFinish={submitLogin}
        requiredMark={false}
        size="large"
      >
        <Form.Item
          label="用户名"
          name="username"
          rules={[
            {
              message: "请输入用户名。",
              required: true,
              whitespace: true,
            },
          ]}
        >
          <Input
            autoComplete="username"
            prefix={<UserOutlined aria-hidden="true" />}
          />
        </Form.Item>

        <Form.Item
          label="密码"
          name="password"
          rules={[
            {
              message: "请输入密码。",
              required: true,
            },
          ]}
        >
          <Input.Password
            autoComplete="current-password"
            prefix={<LockOutlined aria-hidden="true" />}
          />
        </Form.Item>

        {login.isError ? (
          <Alert showIcon type="error" title={toMessage(login.error)} />
        ) : null}

        <Button
          aria-label="登录"
          block
          htmlType="submit"
          icon={<LoginOutlined />}
          loading={login.isPending}
          size="large"
          type="primary"
        >
          登录
        </Button>
      </Form>

      <div className="entry-footer">
        <div className="entry-server-line">
          <Text type="secondary">当前服务器</Text>
          <Text code className="entry-server-code">
            {apiBaseUrl}
          </Text>
        </div>
        <Link to="/server-settings" className="entry-link">
          <SettingOutlined aria-hidden="true" />
          服务器设置
        </Link>
      </div>
    </EntryShell>
  );
}

function toMessage(caught: unknown) {
  if (caught instanceof Error) {
    return caught.message;
  }
  return "登录失败。";
}
