import { KeyRound } from "lucide-react";
import { FormEvent, useState } from "react";
import { NavLink } from "react-router-dom";
import { useChangePassword, useCurrentUser } from "../auth/api";
import { ApiError } from "../../shared/api/httpClient";
import { Button } from "../../shared/ui/Button";
import { TextInput } from "../../shared/ui/TextInput";

export function SettingsPage() {
  const currentUser = useCurrentUser();
  const changePassword = useChangePassword();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPasswordCheck, setNewPasswordCheck] = useState("");
  const [validationError, setValidationError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!currentPassword || !newPassword || !newPasswordCheck) {
      setValidationError("请输入当前密码、新密码和确认密码。");
      setSuccessMessage(null);
      return;
    }
    if (newPassword !== newPasswordCheck) {
      setValidationError("两次输入的新密码不一致。");
      setSuccessMessage(null);
      return;
    }

    setValidationError(null);
    setSuccessMessage(null);
    changePassword.mutate(
      {
        current_password: currentPassword,
        new_password: newPassword,
        new_password_check: newPasswordCheck,
      },
      {
        onSuccess: () => {
          setCurrentPassword("");
          setNewPassword("");
          setNewPasswordCheck("");
          setSuccessMessage("密码已更新。");
        },
      },
    );
  }

  return (
    <>
      <div className="page-header">
        <div>
          <h1>用户设置</h1>
          <p>
            {currentUser.data?.username ?? "未知用户"} ·{" "}
            {currentUser.data?.user_type.name ?? "unknown"}
          </p>
        </div>
      </div>

      <div className="settings-layout">
        <aside className="settings-sidebar" aria-label="用户设置导航">
          <div className="settings-sidebar-title">设置</div>
          <nav className="settings-nav-list">
            <NavLink
              to="/settings/password"
              className={({ isActive }) =>
                isActive ? "settings-nav-link settings-nav-link-active" : "settings-nav-link"
              }
            >
              <KeyRound aria-hidden="true" size={18} />
              <span>密码</span>
            </NavLink>
          </nav>
        </aside>

        <section className="settings-content">
          <div className="settings-section-header">
            <div>
              <h2>密码</h2>
              <p className="muted">保护当前账户的登录凭据。</p>
            </div>
          </div>

          <form className="settings-form panel panel-pad" onSubmit={submit}>
            <label className="form-row">
              <span className="label">当前密码</span>
              <TextInput
                type="password"
                value={currentPassword}
                onChange={(event) => setCurrentPassword(event.target.value)}
                autoComplete="current-password"
              />
            </label>
            <label className="form-row">
              <span className="label">新密码</span>
              <TextInput
                type="password"
                value={newPassword}
                onChange={(event) => setNewPassword(event.target.value)}
                autoComplete="new-password"
              />
            </label>
            <label className="form-row">
              <span className="label">确认新密码</span>
              <TextInput
                type="password"
                value={newPasswordCheck}
                onChange={(event) => setNewPasswordCheck(event.target.value)}
                autoComplete="new-password"
              />
            </label>

            {validationError ? <div className="alert">{validationError}</div> : null}
            {changePassword.isError ? (
              <div className="alert">{getPasswordError(changePassword.error)}</div>
            ) : null}
            {successMessage ? <div className="notice">{successMessage}</div> : null}

            <div className="settings-form-actions">
              <Button type="submit" disabled={changePassword.isPending}>
                {changePassword.isPending ? "更新中..." : "更新密码"}
              </Button>
            </div>
          </form>
        </section>
      </div>
    </>
  );
}

function getPasswordError(error: Error) {
  if (error instanceof ApiError) {
    return error.message;
  }
  return "密码更新失败，请检查服务器连接。";
}
