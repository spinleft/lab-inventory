import {
  ApartmentOutlined,
  LockOutlined,
  SafetyCertificateOutlined,
  SettingOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { Alert, Button, Card, Form, Input, Result, Space, Typography } from "antd";
import { Link } from "react-router-dom";
import { useAppShell } from "../../app/AppShell";
import { useChangePassword } from "../auth/api";
import { type CurrentUser } from "../auth/types";
import {
  canAccessAdminSettings,
  describeRole,
  describeScope,
} from "../auth/permissions";

const { Paragraph, Text, Title } = Typography;

type ChangePasswordFormValues = {
  current_password: string;
  new_password: string;
  new_password_check: string;
};

export function ProfileSettingsPage() {
  const { currentUser } = useAppShell();

  return (
    <Card title="账号信息" className="settings-card">
      <div className="settings-profile-grid">
        <ProfileField label="用户名" value={currentUser.username} />
        <ProfileField label="邮箱" value={currentUser.email ?? "未设置"} />
        <ProfileField label="用户类型" value={describeRole(currentUser)} />
        <ProfileField label="数据范围" value={describeScope(currentUser)} />
      </div>
    </Card>
  );
}

export function PasswordSettingsPage() {
  const [form] = Form.useForm<ChangePasswordFormValues>();
  const changePassword = useChangePassword();

  function submitPasswordChange(values: ChangePasswordFormValues) {
    changePassword.mutate(values, {
      onSuccess: () => {
        form.resetFields();
      },
    });
  }

  return (
    <Card title="修改密码" className="settings-card">
      <Form
        form={form}
        layout="vertical"
        onFinish={submitPasswordChange}
        requiredMark={false}
        className="settings-password-form"
      >
        <Form.Item
          label="当前密码"
          name="current_password"
          rules={[{ required: true, message: "请输入当前密码。" }]}
        >
          <Input.Password
            autoComplete="current-password"
            prefix={<LockOutlined aria-hidden="true" />}
            size="large"
          />
        </Form.Item>
        <Form.Item
          label="新密码"
          name="new_password"
          rules={[{ required: true, message: "请输入新密码。" }]}
        >
          <Input.Password
            autoComplete="new-password"
            prefix={<LockOutlined aria-hidden="true" />}
            size="large"
          />
        </Form.Item>
        <Form.Item
          label="确认新密码"
          name="new_password_check"
          rules={[{ required: true, message: "请再次输入新密码。" }]}
        >
          <Input.Password
            autoComplete="new-password"
            prefix={<LockOutlined aria-hidden="true" />}
            size="large"
          />
        </Form.Item>

        {changePassword.isSuccess ? (
          <Alert showIcon type="success" title="密码已更新。" />
        ) : null}
        {changePassword.isError ? (
          <Alert showIcon type="error" title={toMessage(changePassword.error)} />
        ) : null}

        <Button
          type="primary"
          htmlType="submit"
          size="large"
          loading={changePassword.isPending}
        >
          保存密码
        </Button>
      </Form>
    </Card>
  );
}

export function PreferenceSettingsPage() {
  return (
    <Card className="settings-card">
      <Space align="start">
        <SettingOutlined className="settings-placeholder-icon" aria-hidden="true" />
        <div>
          <Title level={3}>偏好设置</Title>
          <Paragraph type="secondary">
            偏好设置已经接入新的用户设置导航。具体偏好项将在后续切片实现。
          </Paragraph>
          <Text type="secondary">当前仅调整路由和页面框架，不新增配置项。</Text>
        </div>
      </Space>
    </Card>
  );
}

type AdminSection = "overview" | "laboratories" | "users";

export function AdminPage({ section = "overview" }: { section?: AdminSection }) {
  const { currentUser } = useAppShell();
  if (!canAccessAdminSettings(currentUser)) {
    return <SettingsAccessDenied />;
  }

  const isMaintainer = currentUser.user_type.name === "maintainer";
  const sectionContent = getAdminSectionContent(section, currentUser);

  return (
    <Space orientation="vertical" size="large" className="full-width">
      {isMaintainer ? (
        <Alert
          showIcon
          type="info"
          title="你只能管理自己实验室范围内的数据。"
        />
      ) : null}
      <Card className="settings-card">
        <Space align="start">
          {sectionContent.icon}
          <div>
            <Title level={3}>{sectionContent.title}</Title>
            <Paragraph type="secondary">{sectionContent.description}</Paragraph>
            <Text type="secondary">{sectionContent.meta}</Text>
          </div>
        </Space>
      </Card>
    </Space>
  );
}

function getAdminSectionContent(section: AdminSection, currentUser: CurrentUser) {
  const scope =
    currentUser.user_type.name === "maintainer" ? describeScope(currentUser) : "全部实验室";

  if (section === "laboratories") {
    return {
      description: "实验室管理入口已经接入新的管理导航。实验室 CRUD 将在后续切片实现。",
      icon: <ApartmentOutlined className="settings-placeholder-icon" aria-hidden="true" />,
      meta: `当前范围：${scope}`,
      title: "实验室",
    };
  }

  if (section === "users") {
    return {
      description: "用户管理入口已经接入新的管理导航。用户列表和权限管理将在后续切片实现。",
      icon: <UserOutlined className="settings-placeholder-icon" aria-hidden="true" />,
      meta: `当前范围：${scope}`,
      title: "用户",
    };
  }

  return {
    description: "管理中心已经从设置页迁移到独立 /admin 路由。本轮仅调整后台布局和路由结构。",
    icon: <ApartmentOutlined className="settings-placeholder-icon" aria-hidden="true" />,
    meta: `当前范围：${scope}`,
    title: "管理中心",
  };
}

function SettingsAccessDenied() {
  return (
    <Card className="settings-card">
      <Result
        status="403"
        icon={<SafetyCertificateOutlined />}
        title="无权限访问"
        subTitle="当前账号没有访问该设置页面的权限。"
        extra={
          <Link to="/dashboard">
            <Button type="primary">返回概览</Button>
          </Link>
        }
      />
    </Card>
  );
}

function ProfileField({ label, value }: { label: string; value: string }) {
  return (
    <div className="settings-profile-field">
      <Text type="secondary">{label}</Text>
      <Text strong>{value}</Text>
    </div>
  );
}

function toMessage(caught: unknown) {
  if (caught instanceof Error) {
    return caught.message;
  }
  return "操作失败。";
}
