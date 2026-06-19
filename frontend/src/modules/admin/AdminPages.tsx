import { useQueryClient } from "@tanstack/react-query";
import { Building2, Pencil, Plus, Trash2, Users } from "lucide-react";
import { type FormEvent, useEffect, useState } from "react";
import { Link, Navigate } from "react-router-dom";
import { useAuth } from "../../app/auth-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { formatDate } from "../../shared/lib/date";
import { toErrorMessage } from "../../shared/lib/errors";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { ConfirmDialog } from "../../shared/ui/ConfirmDialog";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { Dialog } from "../../shared/ui/Dialog";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { Select } from "../../shared/ui/Select";
import { useToast } from "../../shared/ui/Toast";
import { authQueryKeys } from "../auth/api";
import {
  canManageLaboratories,
  canManageUser,
  getCreatableRoles,
  roleLabel,
  roleRequiresLaboratory,
  roleTone,
} from "../auth/permissions";
import { type UserTypeName } from "../auth/types";
import {
  adminQueryKeys,
  type AdminUser,
  type Laboratory,
  type LaboratoryPayload,
  optionalText,
  useCreateLaboratory,
  useCreateUser,
  useDeleteLaboratory,
  useDeleteUser,
  useLaboratories,
  useUpdateLaboratory,
  useUpdateUser,
  useUsers,
} from "./api";

type LaboratoryForm = {
  address: string;
  contact: string;
  description: string;
  name: string;
};

type UserForm = {
  email: string;
  laboratory_id: string;
  password: string;
  phone_number: string;
  user_type: UserTypeName;
  username: string;
};

export function AdminHomePage() {
  return <Navigate to="/admin/laboratories" replace />;
}

export function LaboratoriesPage() {
  const { currentUser } = useAuth();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const laboratoriesQuery = useLaboratories();
  const createLaboratory = useCreateLaboratory();
  const updateLaboratory = useUpdateLaboratory();
  const deleteLaboratory = useDeleteLaboratory();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Laboratory | "new" | null>(null);
  const isOwner = canManageLaboratories(currentUser);
  const laboratories = laboratoriesQuery.data ?? [];
  const filteredLaboratories = laboratories.filter((laboratory) =>
    [laboratory.name, laboratory.address, laboratory.contact ?? "", laboratory.description ?? ""]
      .join(" ")
      .toLowerCase()
      .includes(search.toLowerCase()),
  );

  function refresh() {
    queryClient.invalidateQueries({ queryKey: adminQueryKeys.laboratories(apiBaseUrl) });
  }

  function handleDelete(laboratory: Laboratory) {
    deleteLaboratory.mutate(laboratory.laboratory_id, {
      onError: (error) =>
        toast.error({ title: "删除实验室失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        refresh();
        toast.success({ title: "实验室已删除" });
      },
    });
  }

  const columns: DataTableColumn<Laboratory>[] = [
    { header: "名称", key: "name", render: (item) => <strong>{item.name}</strong> },
    { header: "地址", key: "address", render: (item) => item.address },
    { header: "联系", key: "contact", render: (item) => item.contact ?? "未设置" },
    { header: "更新时间", key: "updated", render: (item) => formatDate(item.updated_at) },
    {
      align: "right",
      header: "操作",
      key: "actions",
      render: (item) => (
        <span className="table-actions">
          <Button size="icon" variant="ghost" aria-label="编辑" onClick={() => setEditing(item)}>
            <Pencil size={15} />
          </Button>
          {isOwner ? (
            <ConfirmDialog
              confirmLabel="删除"
              description={`确认删除实验室「${item.name}」？`}
              disabled={deleteLaboratory.isPending}
              title="删除实验室"
              trigger={
                <Button size="icon" variant="ghost" aria-label="删除">
                  <Trash2 size={15} />
                </Button>
              }
              onConfirm={() => handleDelete(item)}
            />
          ) : null}
        </span>
      ),
    },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="管理"
        title="实验室"
        description="维护实验室基础信息，作为用户与审计范围的上级边界。"
        actions={
          isOwner ? (
            <Button onClick={() => setEditing("new")} variant="primary">
              <Plus size={15} />
              新建实验室
            </Button>
          ) : null
        }
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">实验室列表</h2>
            <p className="panel-description">{filteredLaboratories.length} 个实验室</p>
          </div>
          <input
            aria-label="搜索实验室"
            className="input"
            placeholder="搜索实验室..."
            style={{ maxWidth: 260 }}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <DataTable
          columns={columns}
          emptyDescription="当前范围内没有实验室。"
          getRowKey={(item) => item.laboratory_id}
          items={filteredLaboratories}
          loading={laboratoriesQuery.isLoading}
        />
      </section>
      <LaboratoryEditor
        laboratory={editing}
        open={editing !== null}
        onClose={() => setEditing(null)}
        onSaved={() => {
          setEditing(null);
          refresh();
        }}
        createLaboratory={createLaboratory}
        updateLaboratory={updateLaboratory}
      />
    </main>
  );
}

function LaboratoryEditor({
  createLaboratory,
  laboratory,
  onClose,
  onSaved,
  open,
  updateLaboratory,
}: {
  createLaboratory: ReturnType<typeof useCreateLaboratory>;
  laboratory: Laboratory | "new" | null;
  onClose: () => void;
  onSaved: () => void;
  open: boolean;
  updateLaboratory: ReturnType<typeof useUpdateLaboratory>;
}) {
  const toast = useToast();
  const isNew = laboratory === "new";
  const [values, setValues] = useState<LaboratoryForm>(emptyLaboratoryForm());
  const isSaving = createLaboratory.isPending || updateLaboratory.isPending;

  useEffect(() => {
    if (!laboratory || laboratory === "new") {
      setValues(emptyLaboratoryForm());
      return;
    }
    setValues({
      address: laboratory.address,
      contact: laboratory.contact ?? "",
      description: laboratory.description ?? "",
      name: laboratory.name,
    });
  }, [laboratory]);

  function updateField(field: keyof LaboratoryForm, value: string) {
    setValues((current) => ({ ...current, [field]: value }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload: LaboratoryPayload = {
      address: values.address.trim(),
      contact: optionalText(values.contact),
      description: optionalText(values.description),
      name: values.name.trim(),
    };
    if (!payload.name || !payload.address) {
      toast.error({ title: "请填写实验室名称和地址" });
      return;
    }

    if (isNew) {
      createLaboratory.mutate(payload, {
        onError: (error) =>
          toast.error({ title: "创建实验室失败", description: toErrorMessage(error) }),
        onSuccess: () => {
          toast.success({ title: "实验室已创建" });
          onSaved();
        },
      });
      return;
    }

    if (laboratory) {
      updateLaboratory.mutate(
        { laboratoryId: laboratory.laboratory_id, payload },
        {
          onError: (error) =>
            toast.error({ title: "更新实验室失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "实验室已更新" });
            onSaved();
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="保存后列表会自动刷新。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "新建实验室" : "编辑实验室"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="laboratory-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="laboratory-form" onSubmit={handleSubmit}>
        <FormField htmlFor="laboratory-name" label="名称">
          <input
            className="input"
            id="laboratory-name"
            value={values.name}
            onChange={(event) => updateField("name", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="laboratory-address" label="地址">
          <input
            className="input"
            id="laboratory-address"
            value={values.address}
            onChange={(event) => updateField("address", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="laboratory-contact" label="联系方式">
          <input
            className="input"
            id="laboratory-contact"
            value={values.contact}
            onChange={(event) => updateField("contact", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="laboratory-description" label="描述">
          <textarea
            className="textarea"
            id="laboratory-description"
            value={values.description}
            onChange={(event) => updateField("description", event.target.value)}
          />
        </FormField>
      </form>
    </Dialog>
  );
}

export function UsersPage() {
  const { currentUser } = useAuth();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const usersQuery = useUsers();
  const laboratoriesQuery = useLaboratories();
  const createUser = useCreateUser();
  const updateUser = useUpdateUser();
  const deleteUser = useDeleteUser();
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<AdminUser | "new" | null>(null);
  const users = usersQuery.data ?? [];
  const laboratories = laboratoriesQuery.data ?? [];
  const filteredUsers = users.filter((user) =>
    [
      user.username,
      user.email ?? "",
      user.phone_number ?? "",
      roleLabel(user.user_type.name),
      user.laboratory?.name ?? "",
    ]
      .join(" ")
      .toLowerCase()
      .includes(search.toLowerCase()),
  );

  function refresh(targetUserId?: string) {
    queryClient.invalidateQueries({ queryKey: adminQueryKeys.users(apiBaseUrl) });
    if (targetUserId === currentUser.user_id) {
      queryClient.invalidateQueries({ queryKey: authQueryKeys.me(apiBaseUrl) });
    }
  }

  function handleDelete(user: AdminUser) {
    deleteUser.mutate(user.user_id, {
      onError: (error) =>
        toast.error({ title: "删除用户失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        refresh();
        toast.success({ title: "用户已删除" });
      },
    });
  }

  const columns: DataTableColumn<AdminUser>[] = [
    { header: "用户名", key: "username", render: (item) => <strong>{item.username}</strong> },
    { header: "邮箱", key: "email", render: (item) => item.email ?? "未设置" },
    { header: "电话", key: "phone", render: (item) => item.phone_number ?? "未设置" },
    {
      header: "角色",
      key: "role",
      render: (item) => <Badge tone={roleTone(item.user_type.name)}>{roleLabel(item.user_type.name)}</Badge>,
    },
    { header: "实验室", key: "lab", render: (item) => item.laboratory?.name ?? "全部" },
    { header: "最近登录", key: "lastLogin", render: (item) => formatDate(item.last_login_at) },
    {
      align: "right",
      header: "操作",
      key: "actions",
      render: (item) => {
        const canEdit = canManageUser(currentUser, item);
        const canDelete = canEdit && item.user_id !== currentUser.user_id;
        return (
          <span className="table-actions">
            <Button
              aria-label="编辑"
              disabled={!canEdit}
              size="icon"
              variant="ghost"
              onClick={() => setEditing(item)}
            >
              <Pencil size={15} />
            </Button>
            {canDelete ? (
              <ConfirmDialog
                confirmLabel="删除"
                description={`确认删除用户「${item.username}」？`}
                disabled={deleteUser.isPending}
                title="删除用户"
                trigger={
                  <Button size="icon" variant="ghost" aria-label="删除">
                    <Trash2 size={15} />
                  </Button>
                }
                onConfirm={() => handleDelete(item)}
              />
            ) : null}
          </span>
        );
      },
    },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="管理"
        title="用户"
        description="管理账号、角色、实验室范围和联系信息。"
        actions={
          <Button onClick={() => setEditing("new")} variant="primary">
            <Plus size={15} />
            新建用户
          </Button>
        }
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">用户列表</h2>
            <p className="panel-description">{filteredUsers.length} 个用户</p>
          </div>
          <input
            aria-label="搜索用户"
            className="input"
            placeholder="搜索用户..."
            style={{ maxWidth: 260 }}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <DataTable
          columns={columns}
          emptyDescription="当前范围内没有用户。"
          getRowKey={(item) => item.user_id}
          items={filteredUsers}
          loading={usersQuery.isLoading || laboratoriesQuery.isLoading}
        />
      </section>
      <UserEditor
        laboratories={laboratories}
        open={editing !== null}
        user={editing}
        onClose={() => setEditing(null)}
        onSaved={(targetUserId) => {
          setEditing(null);
          refresh(targetUserId);
        }}
        createUser={createUser}
        updateUser={updateUser}
      />
    </main>
  );
}

function UserEditor({
  createUser,
  laboratories,
  onClose,
  onSaved,
  open,
  updateUser,
  user,
}: {
  createUser: ReturnType<typeof useCreateUser>;
  laboratories: Laboratory[];
  onClose: () => void;
  onSaved: (targetUserId?: string) => void;
  open: boolean;
  updateUser: ReturnType<typeof useUpdateUser>;
  user: AdminUser | "new" | null;
}) {
  const { currentUser } = useAuth();
  const toast = useToast();
  const isNew = user === "new";
  const roleOptions = getCreatableRoles(currentUser);
  const defaultRole = roleOptions[0] ?? "user";
  const [values, setValues] = useState<UserForm>(emptyUserForm(defaultRole));
  const isSaving = createUser.isPending || updateUser.isPending;
  const isSelf = typeof user === "object" && user?.user_id === currentUser.user_id;

  useEffect(() => {
    if (!user || user === "new") {
      setValues(emptyUserForm(defaultRole, laboratories[0]?.laboratory_id ?? ""));
      return;
    }
    setValues({
      email: user.email ?? "",
      laboratory_id: user.laboratory?.laboratory_id ?? laboratories[0]?.laboratory_id ?? "",
      password: "",
      phone_number: user.phone_number ?? "",
      user_type: user.user_type.name,
      username: user.username,
    });
  }, [defaultRole, laboratories, user]);

  function updateField(field: keyof UserForm, value: string) {
    setValues((current) => {
      if (field === "user_type") {
        const userType = value as UserTypeName;
        return {
          ...current,
          user_type: userType,
          laboratory_id: roleRequiresLaboratory(userType)
            ? current.laboratory_id || laboratories[0]?.laboratory_id || ""
            : "",
        };
      }
      return { ...current, [field]: value };
    });
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const username = values.username.trim();
    const laboratoryId = roleRequiresLaboratory(values.user_type)
      ? values.laboratory_id || null
      : null;
    if (!username) {
      toast.error({ title: "请输入用户名" });
      return;
    }
    if (roleRequiresLaboratory(values.user_type) && !laboratoryId) {
      toast.error({ title: "请选择实验室" });
      return;
    }

    if (isNew) {
      if (!values.password.trim()) {
        toast.error({ title: "请输入初始密码" });
        return;
      }
      createUser.mutate(
        {
          email: optionalText(values.email),
          laboratory_id: laboratoryId,
          password: values.password,
          phone_number: optionalText(values.phone_number),
          user_type: values.user_type,
          username,
        },
        {
          onError: (error) =>
            toast.error({ title: "创建用户失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "用户已创建" });
            onSaved();
          },
        },
      );
      return;
    }

    if (user) {
      updateUser.mutate(
        {
          payload: {
            email: optionalText(values.email),
            phone_number: optionalText(values.phone_number),
            ...(isSelf
              ? {}
              : {
                  laboratory_id: laboratoryId,
                  user_type: values.user_type,
                }),
            username,
          },
          userId: user.user_id,
        },
        {
          onError: (error) =>
            toast.error({ title: "更新用户失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "用户已更新" });
            onSaved(user.user_id);
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description={isNew ? "创建用户后可立即登录。" : "管理员不能在此重置密码。"}
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "新建用户" : "编辑用户"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="user-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="user-form" onSubmit={handleSubmit}>
        <FormField htmlFor="user-username" label="用户名">
          <input
            className="input"
            id="user-username"
            value={values.username}
            onChange={(event) => updateField("username", event.target.value)}
          />
        </FormField>
        <div className="form-grid form-grid-2">
          <FormField htmlFor="user-email" label="邮箱">
            <input
              className="input"
              id="user-email"
              value={values.email}
              onChange={(event) => updateField("email", event.target.value)}
            />
          </FormField>
          <FormField htmlFor="user-phone" label="电话">
            <input
              className="input"
              id="user-phone"
              value={values.phone_number}
              onChange={(event) => updateField("phone_number", event.target.value)}
            />
          </FormField>
        </div>
        {isNew ? (
          <FormField htmlFor="user-password" label="初始密码">
            <input
              autoComplete="new-password"
              className="input"
              id="user-password"
              type="password"
              value={values.password}
              onChange={(event) => updateField("password", event.target.value)}
            />
          </FormField>
        ) : null}
        <div className="form-grid form-grid-2">
          <FormField htmlFor="user-role" label="角色">
            <Select
              disabled={isSelf}
              id="user-role"
              label="角色"
              options={roleOptions.map((role) => ({ label: roleLabel(role), value: role }))}
              value={values.user_type}
              onValueChange={(value) => updateField("user_type", value)}
            />
          </FormField>
          {roleRequiresLaboratory(values.user_type) ? (
            <FormField htmlFor="user-laboratory" label="实验室">
              <Select
                disabled={isSelf}
                id="user-laboratory"
                label="实验室"
                options={laboratories.map((laboratory) => ({
                  label: laboratory.name,
                  value: laboratory.laboratory_id,
                }))}
                value={values.laboratory_id}
                onValueChange={(value) => updateField("laboratory_id", value)}
              />
            </FormField>
          ) : null}
        </div>
      </form>
    </Dialog>
  );
}

function emptyLaboratoryForm(): LaboratoryForm {
  return {
    address: "",
    contact: "",
    description: "",
    name: "",
  };
}

function emptyUserForm(role: UserTypeName, laboratoryId = ""): UserForm {
  return {
    email: "",
    laboratory_id: laboratoryId,
    password: "",
    phone_number: "",
    user_type: role,
    username: "",
  };
}
