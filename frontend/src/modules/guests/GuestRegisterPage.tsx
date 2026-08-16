import { ArrowRight, Server } from "lucide-react";
import { type FormEvent, useState } from "react";
import { Link, Navigate, useNavigate, useSearchParams } from "react-router-dom";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { ApiError } from "../../shared/api/httpClient";
import { toErrorMessage } from "../../shared/lib/errors";
import { Button } from "../../shared/ui/Button";
import { FormField } from "../../shared/ui/FormField";
import { useToast } from "../../shared/ui/Toast";
import { useRegisterGuest } from "./api";

/**
 * Self-service guest registration.
 *
 * Open without a session — the visitor has a code, not an account — so it sits
 * beside the login page rather than inside the shell. Registering does not sign
 * anyone in; the API answers with the new user and nothing else, so this hands
 * off to the login page.
 */
export function GuestRegisterPage() {
  const { hasConfiguredApiBaseUrl } = useBackendConfig();
  const [searchParams] = useSearchParams();
  const register = useRegisterGuest();
  const navigate = useNavigate();
  const toast = useToast();
  const [values, setValues] = useState({
    description: "",
    email: "",
    password: "",
    phone_number: "",
    // A code pasted into the address bar saves the visitor retyping it.
    registration_code: searchParams.get("code") ?? "",
    username: "",
  });

  if (!hasConfiguredApiBaseUrl) {
    return <Navigate to="/server-settings" replace />;
  }

  // The note is the one optional field, so it stays out of this check.
  const complete = Object.entries(values)
    .filter(([field]) => field !== "description")
    .every(([, value]) => value.trim());

  function update(field: keyof typeof values, value: string) {
    setValues((current) => ({ ...current, [field]: value }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    register.mutate(
      {
        description: values.description.trim() || null,
        email: values.email.trim(),
        password: values.password,
        phone_number: values.phone_number.trim(),
        registration_code: values.registration_code.trim(),
        username: values.username.trim(),
      },
      {
        onError: (error) => {
          // The endpoint is rate limited, and "请求失败" would leave the
          // visitor retrying into the same wall.
          const description =
            error instanceof ApiError && error.status === 429
              ? "尝试过于频繁，请稍后再试。"
              : toErrorMessage(error);
          toast.error({ title: "注册失败", description });
        },
        onSuccess: () => {
          toast.success({
            title: "注册成功",
            description: "请用刚设置的用户名和密码登录。",
          });
          navigate("/login", { replace: true });
        },
      },
    );
  }

  return (
    <main className="entry-page">
      <div className="entry-shell entry-shell-compact">
        <section className="entry-card entry-card-compact" aria-label="访客注册表单">
          <div className="entry-card-inner">
            <div className="entry-brand">
              <span className="brand-mark">LI</span>
              <span>Lab Inventory</span>
            </div>
            <h1 className="entry-compact-title">注册访客账号</h1>
            <p className="entry-description">
              向实验室成员索取注册码，注册后可以只读浏览该实验室的资产与库存。
            </p>
            <form className="entry-form" onSubmit={handleSubmit}>
              <FormField htmlFor="register-code" label="注册码">
                <input
                  autoComplete="one-time-code"
                  className="input"
                  id="register-code"
                  value={values.registration_code}
                  onChange={(event) => update("registration_code", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="register-username" label="用户名">
                <input
                  autoComplete="username"
                  className="input"
                  id="register-username"
                  value={values.username}
                  onChange={(event) => update("username", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="register-password" label="密码">
                <input
                  autoComplete="new-password"
                  className="input"
                  id="register-password"
                  type="password"
                  value={values.password}
                  onChange={(event) => update("password", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="register-email" label="邮箱">
                <input
                  autoComplete="email"
                  className="input"
                  id="register-email"
                  type="email"
                  value={values.email}
                  onChange={(event) => update("email", event.target.value)}
                />
              </FormField>
              <FormField htmlFor="register-phone" label="手机号">
                <input
                  autoComplete="tel"
                  className="input"
                  id="register-phone"
                  value={values.phone_number}
                  onChange={(event) => update("phone_number", event.target.value)}
                />
              </FormField>
              <FormField
                hint="选填。写一句你是谁、来做什么，实验室管理员会看到。"
                htmlFor="register-description"
                label="备注"
              >
                <textarea
                  className="input"
                  id="register-description"
                  rows={2}
                  value={values.description}
                  onChange={(event) => update("description", event.target.value)}
                />
              </FormField>
              <div className="entry-actions">
                <Button asChild>
                  <Link to="/login">
                    <Server size={15} />
                    返回登录
                  </Link>
                </Button>
                <Button
                  disabled={!complete || register.isPending}
                  type="submit"
                  variant="primary"
                >
                  注册
                  <ArrowRight size={15} />
                </Button>
              </div>
            </form>
          </div>
        </section>
      </div>
    </main>
  );
}
