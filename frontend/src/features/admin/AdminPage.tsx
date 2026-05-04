import {
  Building2,
  Pencil,
  Plus,
  Save,
  Search,
  ShieldCheck,
  Trash2,
  Users,
  X,
} from "lucide-react";
import { type FormEvent, useMemo, useState } from "react";
import { NavLink } from "react-router-dom";
import { useCurrentUser } from "../auth/api";
import {
  canAccessAdminArea,
  canManageLaboratories,
  isAdminUser,
  isMaintainerUser,
  userTypeBadgeClass,
  userTypeLabel,
} from "../auth/permissions";
import { type CurrentUser } from "../auth/types";
import { Button } from "../../shared/ui/Button";
import { NativeSelect } from "../../shared/ui/NativeSelect";
import { TextInput } from "../../shared/ui/TextInput";
import {
  type CreateLaboratoryInput,
  type CreateUserInput,
  type Laboratory,
  type ManagedUser,
  type UpdateLaboratoryInput,
  type UpdateUserInput,
  useCreateLaboratory,
  useCreateUser,
  useDeleteLaboratory,
  useDeleteUser,
  useLaboratories,
  useUpdateLaboratory,
  useUpdateUser,
  useUsers,
} from "./api";

export type AdminSection = "users" | "laboratories";

type FlashMessage = {
  type: "notice" | "alert";
  text: string;
};

const ADMIN_USER_TYPES = ["owner", "maintainer", "user", "guest"] as const;
const MAINTAINER_USER_TYPES = ["user", "guest"] as const;

export function AdminPage({ section }: { section: AdminSection }) {
  const currentUser = useCurrentUser();
  const user = currentUser.data;
  const canUseAdminArea = canAccessAdminArea(user);
  const canUseLaboratories = canManageLaboratories(user);

  if (currentUser.isLoading) {
    return <p className="muted">正在加载管理区...</p>;
  }

  if (!user || !canUseAdminArea) {
    return <AdminAccessDenied />;
  }

  if (section === "laboratories" && !canUseLaboratories) {
    return (
      <AdminFrame currentUser={user} section={section}>
        <section className="problem-panel admin-problem">
          <h2>无法访问实验室管理</h2>
          <p className="muted">只有管理员可以新建、修改和删除实验室。</p>
        </section>
      </AdminFrame>
    );
  }

  return (
    <AdminFrame currentUser={user} section={section}>
      {section === "users" ? (
        <UsersManagement currentUser={user} />
      ) : (
        <LaboratoriesManagement />
      )}
    </AdminFrame>
  );
}

function AdminFrame({
  children,
  currentUser,
  section,
}: {
  children: React.ReactNode;
  currentUser: CurrentUser;
  section: AdminSection;
}) {
  const showLaboratories = canManageLaboratories(currentUser);

  return (
    <>
      <div className="page-header">
        <div>
          <h1>管理区</h1>
          <p>
            {userTypeLabel(currentUser.user_type.name)} ·{" "}
            {currentUser.laboratory?.name ?? "全部实验室"}
          </p>
        </div>
      </div>

      <div className="admin-layout">
        <aside className="admin-sidebar" aria-label="管理区导航">
          <div className="admin-sidebar-title">Admin Area</div>
          <nav className="admin-nav-list">
            <NavLink
              to="/admin/users"
              className={({ isActive }) =>
                isActive || section === "users"
                  ? "admin-nav-link admin-nav-link-active"
                  : "admin-nav-link"
              }
            >
              <Users aria-hidden="true" size={18} />
              <span>用户</span>
            </NavLink>
            {showLaboratories ? (
              <NavLink
                to="/admin/laboratories"
                className={({ isActive }) =>
                  isActive || section === "laboratories"
                    ? "admin-nav-link admin-nav-link-active"
                    : "admin-nav-link"
                }
              >
                <Building2 aria-hidden="true" size={18} />
                <span>实验室</span>
              </NavLink>
            ) : null}
          </nav>
        </aside>

        <div className="admin-content">{children}</div>
      </div>
    </>
  );
}

function AdminAccessDenied() {
  return (
    <>
      <div className="page-header">
        <div>
          <h1>管理区</h1>
          <p>当前账户没有管理权限。</p>
        </div>
      </div>
      <section className="problem-panel admin-problem">
        <h2>无法访问管理区</h2>
        <p className="muted">请使用管理员或实验室维护者账户登录。</p>
      </section>
    </>
  );
}

function UsersManagement({ currentUser }: { currentUser: CurrentUser }) {
  const [query, setQuery] = useState("");
  const [editingUser, setEditingUser] = useState<ManagedUser | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [flash, setFlash] = useState<FlashMessage | null>(null);
  const users = useUsers();
  const laboratories = useLaboratories(isAdminUser(currentUser));
  const createUser = useCreateUser();
  const updateUser = useUpdateUser();
  const deleteUser = useDeleteUser();

  const filteredUsers = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    const items = users.data ?? [];
    if (!normalizedQuery) {
      return items;
    }

    return items.filter((user) =>
      [
        user.username,
        user.email ?? "",
        user.user_type.name,
        userTypeLabel(user.user_type.name),
        user.laboratory?.name ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [query, users.data]);

  function openCreateForm() {
    setEditingUser(null);
    setShowForm(true);
    setFlash(null);
  }

  function openEditForm(user: ManagedUser) {
    setEditingUser(user);
    setShowForm(true);
    setFlash(null);
  }

  function closeForm() {
    setEditingUser(null);
    setShowForm(false);
  }

  function submitUser(input: CreateUserInput | UpdateUserInput) {
    setFlash(null);
    if ("user_id" in input) {
      updateUser.mutate(input, {
        onSuccess: () => {
          setFlash({ type: "notice", text: "用户已更新。" });
          closeForm();
        },
        onError: (error) =>
          setFlash({ type: "alert", text: errorMessage(error, "用户更新失败。") }),
      });
      return;
    }

    createUser.mutate(input, {
      onSuccess: () => {
        setFlash({ type: "notice", text: "用户已创建。" });
        closeForm();
      },
      onError: (error) =>
        setFlash({ type: "alert", text: errorMessage(error, "用户创建失败。") }),
    });
  }

  function removeUser(user: ManagedUser) {
    if (!window.confirm(`删除用户 ${user.username}？`)) {
      return;
    }

    setFlash(null);
    deleteUser.mutate(user.user_id, {
      onSuccess: () => setFlash({ type: "notice", text: "用户已删除。" }),
      onError: (error) =>
        setFlash({ type: "alert", text: errorMessage(error, "用户删除失败。") }),
    });
  }

  const pending =
    createUser.isPending || updateUser.isPending || deleteUser.isPending;

  return (
    <>
      <div className="admin-section-header">
        <div>
          <h2>用户</h2>
          <p className="muted">
            {isAdminUser(currentUser)
              ? "管理员可管理所有用户。"
              : `${currentUser.laboratory?.name ?? "当前实验室"} 用户`}
          </p>
        </div>
        <Button type="button" onClick={openCreateForm}>
          <Plus aria-hidden="true" size={16} />
          新建用户
        </Button>
      </div>

      {flash ? <div className={flash.type}>{flash.text}</div> : null}

      {showForm ? (
        <UserForm
          key={editingUser?.user_id ?? "new-user"}
          currentUser={currentUser}
          editingUser={editingUser}
          laboratories={laboratories.data ?? []}
          laboratoriesLoading={laboratories.isLoading}
          pending={pending}
          onCancel={closeForm}
          onSubmit={submitUser}
        />
      ) : null}

      <section className="panel panel-pad">
        <div className="filters admin-table-toolbar">
          <label className="search-field">
            <Search aria-hidden="true" size={18} />
            <TextInput
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索用户、邮箱、实验室"
              aria-label="搜索用户"
            />
          </label>
        </div>

        {users.isLoading ? <p className="muted">正在加载用户...</p> : null}
        {users.isError ? <div className="alert">{users.error.message}</div> : null}
        {laboratories.isError ? (
          <div className="alert">{laboratories.error.message}</div>
        ) : null}

        {users.data ? (
          <>
            <div className="table-wrap">
              <table className="data-table admin-data-table">
                <thead>
                  <tr>
                    <th>用户</th>
                    <th>类型</th>
                    <th>实验室</th>
                    <th>上次登录</th>
                    <th>创建时间</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredUsers.map((managedUser) => {
                    const editable = canManageUserRow(currentUser, managedUser);
                    const deletable =
                      editable && managedUser.user_id !== currentUser.user_id;
                    return (
                      <tr key={managedUser.user_id}>
                        <td>
                          <strong>{managedUser.username}</strong>
                          <p className="muted small">{managedUser.email || "-"}</p>
                        </td>
                        <td>
                          <span className={userTypeBadgeClass(managedUser.user_type.name)}>
                            {userTypeLabel(managedUser.user_type.name)}
                          </span>
                        </td>
                        <td>{managedUser.laboratory?.name ?? "全部实验室"}</td>
                        <td>{formatDate(managedUser.last_login_at)}</td>
                        <td>{formatDate(managedUser.created_at)}</td>
                        <td>
                          <div className="table-actions">
                            <Button
                              type="button"
                              variant="secondary"
                              onClick={() => openEditForm(managedUser)}
                              disabled={!editable || pending}
                            >
                              <Pencil aria-hidden="true" size={15} />
                              修改
                            </Button>
                            <Button
                              type="button"
                              variant="secondary"
                              className="button-danger"
                              onClick={() => removeUser(managedUser)}
                              disabled={!deletable || pending}
                            >
                              <Trash2 aria-hidden="true" size={15} />
                              删除
                            </Button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            {filteredUsers.length === 0 ? (
              <p className="muted table-empty">暂无用户。</p>
            ) : null}
          </>
        ) : null}
      </section>
    </>
  );
}

function UserForm({
  currentUser,
  editingUser,
  laboratories,
  laboratoriesLoading,
  onCancel,
  onSubmit,
  pending,
}: {
  currentUser: CurrentUser;
  editingUser: ManagedUser | null;
  laboratories: Laboratory[];
  laboratoriesLoading: boolean;
  onCancel: () => void;
  onSubmit: (input: CreateUserInput | UpdateUserInput) => void;
  pending: boolean;
}) {
  const admin = isAdminUser(currentUser);
  const typeOptions = admin ? ADMIN_USER_TYPES : MAINTAINER_USER_TYPES;
  const initialUserType = normalizeEditableUserType(
    editingUser?.user_type.name,
    admin,
  );
  const [username, setUsername] = useState(editingUser?.username ?? "");
  const [email, setEmail] = useState(editingUser?.email ?? "");
  const [password, setPassword] = useState("");
  const [userType, setUserType] = useState<string>(initialUserType);
  const [laboratoryId, setLaboratoryId] = useState(
    editingUser?.laboratory?.laboratory_id ?? currentUser.laboratory?.laboratory_id ?? "",
  );
  const [validationError, setValidationError] = useState<string | null>(null);
  const labRequired = requiresLaboratory(userType);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedUsername = username.trim();
    const trimmedEmail = email.trim();

    if (!trimmedUsername) {
      setValidationError("请输入用户名。");
      return;
    }

    if (!editingUser && !password) {
      setValidationError("请输入初始密码。");
      return;
    }

    if (admin && labRequired && !laboratoryId) {
      setValidationError("请选择实验室。");
      return;
    }

    if (isMaintainerUser(currentUser) && !currentUser.laboratory) {
      setValidationError("当前维护者账户未绑定实验室。");
      return;
    }

    setValidationError(null);

    if (editingUser) {
      const input: UpdateUserInput = {
        user_id: editingUser.user_id,
        username: trimmedUsername,
        user_type: admin ? userType : normalizeMaintainerTargetType(userType),
        email: trimmedEmail,
      };
      if (password) {
        input.password = password;
      }
      if (admin) {
        input.laboratory_id = labRequired ? laboratoryId : null;
      }
      onSubmit(input);
      return;
    }

    const input: CreateUserInput = {
      username: trimmedUsername,
      password,
      user_type: admin ? userType : normalizeMaintainerTargetType(userType),
    };
    if (trimmedEmail) {
      input.email = trimmedEmail;
    }
    if (admin && labRequired) {
      input.laboratory_id = laboratoryId;
    }
    onSubmit(input);
  }

  return (
    <form className="panel panel-pad admin-form" onSubmit={submit}>
      <div className="admin-form-title">
        <div>
          <h3>{editingUser ? "修改用户" : "新建用户"}</h3>
          <p className="muted small">
            {editingUser ? editingUser.username : "创建登录账户"}
          </p>
        </div>
        <Button type="button" variant="ghost" onClick={onCancel}>
          <X aria-hidden="true" size={16} />
          关闭
        </Button>
      </div>

      <div className="form-grid">
        <label className="form-row">
          <span className="label">用户名</span>
          <TextInput
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            autoComplete="username"
          />
        </label>
        <label className="form-row">
          <span className="label">邮箱</span>
          <TextInput
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            autoComplete="email"
          />
        </label>
        <label className="form-row">
          <span className="label">{editingUser ? "新密码" : "初始密码"}</span>
          <TextInput
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="new-password"
            placeholder={editingUser ? "留空则不修改" : undefined}
          />
        </label>
        <label className="form-row">
          <span className="label">用户类型</span>
          <NativeSelect
            value={userType}
            onChange={(event) => {
              const nextUserType = event.target.value;
              setUserType(nextUserType);
              if (!requiresLaboratory(nextUserType)) {
                setLaboratoryId("");
              }
            }}
          >
            {typeOptions.map((value) => (
              <option key={value} value={value}>
                {userTypeLabel(value)}
              </option>
            ))}
          </NativeSelect>
        </label>
        {admin ? (
          <label className="form-row">
            <span className="label">实验室</span>
            <NativeSelect
              value={laboratoryId}
              onChange={(event) => setLaboratoryId(event.target.value)}
              disabled={!labRequired || laboratoriesLoading}
            >
              <option value="">
                {labRequired ? "选择实验室" : "不绑定实验室"}
              </option>
              {laboratories.map((laboratory) => (
                <option
                  key={laboratory.laboratory_id}
                  value={laboratory.laboratory_id}
                >
                  {laboratory.name}
                </option>
              ))}
            </NativeSelect>
          </label>
        ) : (
          <label className="form-row">
            <span className="label">实验室</span>
            <TextInput
              value={currentUser.laboratory?.name ?? "未绑定实验室"}
              disabled
            />
          </label>
        )}
      </div>

      {validationError ? <div className="alert">{validationError}</div> : null}

      <div className="settings-form-actions">
        <Button type="submit" disabled={pending}>
          <Save aria-hidden="true" size={16} />
          {pending ? "保存中..." : editingUser ? "保存修改" : "创建用户"}
        </Button>
      </div>
    </form>
  );
}

function LaboratoriesManagement() {
  const [query, setQuery] = useState("");
  const [editingLaboratory, setEditingLaboratory] = useState<Laboratory | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [flash, setFlash] = useState<FlashMessage | null>(null);
  const laboratories = useLaboratories();
  const createLaboratory = useCreateLaboratory();
  const updateLaboratory = useUpdateLaboratory();
  const deleteLaboratory = useDeleteLaboratory();

  const filteredLaboratories = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    const items = laboratories.data ?? [];
    if (!normalizedQuery) {
      return items;
    }

    return items.filter((laboratory) =>
      [
        laboratory.name,
        laboratory.address,
        laboratory.contact ?? "",
        laboratory.description ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery),
    );
  }, [laboratories.data, query]);

  function openCreateForm() {
    setEditingLaboratory(null);
    setShowForm(true);
    setFlash(null);
  }

  function openEditForm(laboratory: Laboratory) {
    setEditingLaboratory(laboratory);
    setShowForm(true);
    setFlash(null);
  }

  function closeForm() {
    setEditingLaboratory(null);
    setShowForm(false);
  }

  function submitLaboratory(input: CreateLaboratoryInput | UpdateLaboratoryInput) {
    setFlash(null);
    if ("laboratory_id" in input) {
      updateLaboratory.mutate(input, {
        onSuccess: () => {
          setFlash({ type: "notice", text: "实验室已更新。" });
          closeForm();
        },
        onError: (error) =>
          setFlash({
            type: "alert",
            text: errorMessage(error, "实验室更新失败。"),
          }),
      });
      return;
    }

    createLaboratory.mutate(input, {
      onSuccess: () => {
        setFlash({ type: "notice", text: "实验室已创建。" });
        closeForm();
      },
      onError: (error) =>
        setFlash({
          type: "alert",
          text: errorMessage(error, "实验室创建失败。"),
        }),
    });
  }

  function removeLaboratory(laboratory: Laboratory) {
    if (!window.confirm(`删除实验室 ${laboratory.name}？`)) {
      return;
    }

    setFlash(null);
    deleteLaboratory.mutate(laboratory.laboratory_id, {
      onSuccess: () => setFlash({ type: "notice", text: "实验室已删除。" }),
      onError: (error) =>
        setFlash({
          type: "alert",
          text: errorMessage(error, "实验室删除失败。"),
        }),
    });
  }

  const pending =
    createLaboratory.isPending ||
    updateLaboratory.isPending ||
    deleteLaboratory.isPending;

  return (
    <>
      <div className="admin-section-header">
        <div>
          <h2>实验室</h2>
          <p className="muted">管理员可管理实验室档案。</p>
        </div>
        <Button type="button" onClick={openCreateForm}>
          <Plus aria-hidden="true" size={16} />
          新建实验室
        </Button>
      </div>

      {flash ? <div className={flash.type}>{flash.text}</div> : null}

      {showForm ? (
        <LaboratoryForm
          key={editingLaboratory?.laboratory_id ?? "new-laboratory"}
          editingLaboratory={editingLaboratory}
          pending={pending}
          onCancel={closeForm}
          onSubmit={submitLaboratory}
        />
      ) : null}

      <section className="panel panel-pad">
        <div className="filters admin-table-toolbar">
          <label className="search-field">
            <Search aria-hidden="true" size={18} />
            <TextInput
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索实验室、地址、联系人"
              aria-label="搜索实验室"
            />
          </label>
        </div>

        {laboratories.isLoading ? <p className="muted">正在加载实验室...</p> : null}
        {laboratories.isError ? (
          <div className="alert">{laboratories.error.message}</div>
        ) : null}

        {laboratories.data ? (
          <>
            <div className="table-wrap">
              <table className="data-table admin-data-table">
                <thead>
                  <tr>
                    <th>实验室</th>
                    <th>地址</th>
                    <th>联系人</th>
                    <th>更新时间</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredLaboratories.map((laboratory) => (
                    <tr key={laboratory.laboratory_id}>
                      <td>
                        <strong>{laboratory.name}</strong>
                        <p className="muted small">
                          {laboratory.description || "-"}
                        </p>
                      </td>
                      <td>{laboratory.address}</td>
                      <td>{laboratory.contact || "-"}</td>
                      <td>{formatDate(laboratory.updated_at)}</td>
                      <td>
                        <div className="table-actions">
                          <Button
                            type="button"
                            variant="secondary"
                            onClick={() => openEditForm(laboratory)}
                            disabled={pending}
                          >
                            <Pencil aria-hidden="true" size={15} />
                            修改
                          </Button>
                          <Button
                            type="button"
                            variant="secondary"
                            className="button-danger"
                            onClick={() => removeLaboratory(laboratory)}
                            disabled={pending}
                          >
                            <Trash2 aria-hidden="true" size={15} />
                            删除
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {filteredLaboratories.length === 0 ? (
              <p className="muted table-empty">暂无实验室。</p>
            ) : null}
          </>
        ) : null}
      </section>
    </>
  );
}

function LaboratoryForm({
  editingLaboratory,
  onCancel,
  onSubmit,
  pending,
}: {
  editingLaboratory: Laboratory | null;
  onCancel: () => void;
  onSubmit: (input: CreateLaboratoryInput | UpdateLaboratoryInput) => void;
  pending: boolean;
}) {
  const [name, setName] = useState(editingLaboratory?.name ?? "");
  const [address, setAddress] = useState(editingLaboratory?.address ?? "");
  const [description, setDescription] = useState(
    editingLaboratory?.description ?? "",
  );
  const [contact, setContact] = useState(editingLaboratory?.contact ?? "");
  const [validationError, setValidationError] = useState<string | null>(null);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedName = name.trim();
    const trimmedAddress = address.trim();

    if (!trimmedName || !trimmedAddress) {
      setValidationError("请输入实验室名称和地址。");
      return;
    }

    setValidationError(null);
    const input = {
      name: trimmedName,
      address: trimmedAddress,
      description: description.trim(),
      contact: contact.trim(),
    };

    if (editingLaboratory) {
      onSubmit({
        laboratory_id: editingLaboratory.laboratory_id,
        ...input,
      });
      return;
    }

    onSubmit(input);
  }

  return (
    <form className="panel panel-pad admin-form" onSubmit={submit}>
      <div className="admin-form-title">
        <div>
          <h3>{editingLaboratory ? "修改实验室" : "新建实验室"}</h3>
          <p className="muted small">
            {editingLaboratory ? editingLaboratory.name : "登记实验室档案"}
          </p>
        </div>
        <Button type="button" variant="ghost" onClick={onCancel}>
          <X aria-hidden="true" size={16} />
          关闭
        </Button>
      </div>

      <div className="form-grid">
        <label className="form-row">
          <span className="label">名称</span>
          <TextInput
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="form-row">
          <span className="label">地址</span>
          <TextInput
            value={address}
            onChange={(event) => setAddress(event.target.value)}
          />
        </label>
        <label className="form-row">
          <span className="label">描述</span>
          <TextInput
            value={description}
            onChange={(event) => setDescription(event.target.value)}
          />
        </label>
        <label className="form-row">
          <span className="label">联系人</span>
          <TextInput
            value={contact}
            onChange={(event) => setContact(event.target.value)}
          />
        </label>
      </div>

      {validationError ? <div className="alert">{validationError}</div> : null}

      <div className="settings-form-actions">
        <Button type="submit" disabled={pending}>
          <Save aria-hidden="true" size={16} />
          {pending ? "保存中..." : editingLaboratory ? "保存修改" : "创建实验室"}
        </Button>
      </div>
    </form>
  );
}

function canManageUserRow(currentUser: CurrentUser, managedUser: ManagedUser) {
  if (isAdminUser(currentUser)) {
    return true;
  }

  return (
    isMaintainerUser(currentUser) &&
    managedUser.laboratory?.laboratory_id === currentUser.laboratory?.laboratory_id &&
    (managedUser.user_type.name === "user" || managedUser.user_type.name === "guest")
  );
}

function normalizeEditableUserType(
  userType: string | undefined,
  admin: boolean,
) {
  if (admin) {
    return userType === "admin" ? "owner" : userType ?? "user";
  }
  return userType === "guest" ? "guest" : "user";
}

function normalizeMaintainerTargetType(userType: string) {
  return userType === "guest" ? "guest" : "user";
}

function requiresLaboratory(userType: string) {
  return userType === "maintainer" || userType === "user" || userType === "guest";
}

function formatDate(value: string | null) {
  if (!value) {
    return "-";
  }

  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function errorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message;
  }
  return fallback;
}

export function AdminNavIcon() {
  return <ShieldCheck aria-hidden="true" size={18} />;
}
