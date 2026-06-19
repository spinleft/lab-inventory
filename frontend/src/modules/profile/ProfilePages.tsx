import { useQueryClient } from "@tanstack/react-query";
import { Check, KeyRound } from "lucide-react";
import { type FormEvent, useState } from "react";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { Button } from "../../shared/ui/Button";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { useToast } from "../../shared/ui/Toast";
import { useTheme, type ThemePreference } from "../../shared/theme/ThemeProvider";
import { toErrorMessage } from "../../shared/lib/errors";
import { useAuth } from "../../app/auth-context";
import { authQueryKeys, useChangePassword } from "../auth/api";
import { describeRole, describeScope } from "../auth/permissions";

export function ProfilePage() {
  const { currentUser } = useAuth();

  return (
    <main className="page">
      <PageHeader
        kicker="账号"
        title="个人资料"
        description="当前登录账号和权限范围。"
      />
      <section className="panel">
        <div className="panel-body">
          <div className="profile-grid">
            <ProfileField label="用户名" value={currentUser.username} />
            <ProfileField label="邮箱" value={currentUser.email ?? "未设置"} />
            <ProfileField label="角色" value={describeRole(currentUser)} />
            <ProfileField label="数据范围" value={describeScope(currentUser)} />
          </div>
        </div>
      </section>
    </main>
  );
}

export function PasswordPage() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const changePassword = useChangePassword();
  const toast = useToast();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPasswordCheck, setNewPasswordCheck] = useState("");

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    changePassword.mutate(
      {
        current_password: currentPassword,
        new_password: newPassword,
        new_password_check: newPasswordCheck,
      },
      {
        onError: (error) => toast.error({ title: "密码修改失败", description: toErrorMessage(error) }),
        onSuccess: async () => {
          setCurrentPassword("");
          setNewPassword("");
          setNewPasswordCheck("");
          await queryClient.invalidateQueries({ queryKey: authQueryKeys.me(apiBaseUrl) });
          toast.success({ title: "密码已更新" });
        },
      },
    );
  }

  return (
    <main className="page">
      <PageHeader kicker="安全" title="修改密码" description="修改当前登录账号的密码。" />
      <section className="panel">
        <div className="panel-body">
          <form className="form-grid" style={{ maxWidth: 520 }} onSubmit={handleSubmit}>
            <FormField htmlFor="current-password" label="当前密码">
              <input
                autoComplete="current-password"
                className="input"
                id="current-password"
                type="password"
                value={currentPassword}
                onChange={(event) => setCurrentPassword(event.target.value)}
              />
            </FormField>
            <FormField htmlFor="new-password" label="新密码">
              <input
                autoComplete="new-password"
                className="input"
                id="new-password"
                type="password"
                value={newPassword}
                onChange={(event) => setNewPassword(event.target.value)}
              />
            </FormField>
            <FormField htmlFor="new-password-check" label="确认新密码">
              <input
                autoComplete="new-password"
                className="input"
                id="new-password-check"
                type="password"
                value={newPasswordCheck}
                onChange={(event) => setNewPasswordCheck(event.target.value)}
              />
            </FormField>
            <div>
              <Button
                disabled={
                  !currentPassword ||
                  !newPassword ||
                  !newPasswordCheck ||
                  changePassword.isPending
                }
                type="submit"
                variant="primary"
              >
                <KeyRound size={15} />
                保存密码
              </Button>
            </div>
          </form>
        </div>
      </section>
    </main>
  );
}

export function PreferencesPage() {
  const { preference, setPreference } = useTheme();
  const options: Array<{ label: string; value: ThemePreference }> = [
    { label: "跟随系统", value: "system" },
    { label: "浅色", value: "light" },
    { label: "深色", value: "dark" },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="体验"
        title="偏好设置"
        description="首版仅提供主题偏好。后续模块可以在这里注册自己的设置分组。"
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">主题</h2>
            <p className="panel-description">选择应用的亮暗模式。</p>
          </div>
        </div>
        <div className="panel-body">
          <div className="toolbar-group">
            {options.map((option) => (
              <Button
                key={option.value}
                onClick={() => setPreference(option.value)}
                variant={preference === option.value ? "primary" : "default"}
              >
                {preference === option.value ? <Check size={15} /> : null}
                {option.label}
              </Button>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}

function ProfileField({ label, value }: { label: string; value: string }) {
  return (
    <div className="profile-field">
      <p className="profile-field-label">{label}</p>
      <p className="profile-field-value">{value}</p>
    </div>
  );
}
