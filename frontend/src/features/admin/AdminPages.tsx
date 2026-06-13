import {
  ApartmentOutlined,
  ApiOutlined,
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  SafetyCertificateOutlined,
  UserOutlined,
} from "@ant-design/icons";
import { useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  App as AntApp,
  Button,
  Card,
  Drawer,
  Form,
  Input,
  Popconfirm,
  Radio,
  Result,
  Select,
  Space,
  Switch,
  Table,
  Tag,
  Typography,
  type TableProps,
} from "antd";
import { type FormEvent, useState } from "react";
import { Link } from "react-router-dom";
import { useAppShell } from "../../app/AppShell";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { canAccessAdminSettings, describeScope } from "../auth/permissions";
import { type CurrentUser } from "../auth/types";
import {
  adminQueryKeys,
  type AdminUser,
  type CreateUserPayload,
  type Laboratory,
  type LaboratoryPayload,
  type RemoteInventoryItem,
  type RemoteLaboratory,
  type RemoteLaboratoryPayload,
  type UpdateUserPayload,
  useCreateLaboratory,
  useCreateRemoteBorrowRequest,
  useCreateRemoteLaboratory,
  useCreateUser,
  useDeleteLaboratory,
  useDeleteUser,
  useLaboratories,
  useRemoteInventory,
  useRemoteLaboratories,
  useUpdateLaboratory,
  useUpdateUser,
  useUsers,
} from "./api";

const { Paragraph, Text, Title } = Typography;

type AdminSection = "overview" | "laboratories" | "users" | "remotes";
type RemoteSectionFormValues = {
  name: string;
  api_base_url: string;
  key_id: string;
  shared_secret: string;
  is_enabled: boolean;
};
type LaboratoryFormValues = {
  name: string;
  address: string;
  description: string;
  contact: string;
};
type LaboratoryDrawerMode = "create" | "edit";
type LaboratoryFormErrors = Partial<Record<"name" | "address", string>>;
type UserFormValues = {
  username: string;
  email: string;
  password: string;
  user_type: string;
  laboratory_id: string;
};
type UserDrawerMode = "create" | "edit";
type UserFormErrors = Partial<Record<"username" | "password" | "laboratory_id", string>>;

export function AdminPage({ section = "overview" }: { section?: AdminSection }) {
  const { currentUser } = useAppShell();
  if (!canAccessAdminSettings(currentUser)) {
    return <AdminAccessDenied />;
  }

  const sectionContent = getAdminSectionContent(section, currentUser);

  return (
    <Space orientation="vertical" size="large" className="full-width">
      {section === "laboratories" ? (
        <LaboratoriesSection currentUser={currentUser} />
      ) : section === "users" ? (
        <UsersSection currentUser={currentUser} />
      ) : section === "remotes" ? (
        <RemoteLaboratoriesSection />
      ) : (
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
      )}
    </Space>
  );
}

function getAdminSectionContent(section: AdminSection, currentUser: CurrentUser) {
  const scope = currentUser.user_type.name === "admin" ? "本地节点" : describeScope(currentUser);

  if (section === "laboratories") {
    return {
      description: "维护实验室基础信息，作为库存、人员和审计范围的上级边界。",
      icon: <ApartmentOutlined className="settings-placeholder-icon" aria-hidden="true" />,
      meta: `当前范围：${scope}`,
      title: "实验室",
    };
  }

  if (section === "users") {
    return {
      description: "管理用户、角色和实验室范围。",
      icon: <UserOutlined className="settings-placeholder-icon" aria-hidden="true" />,
      meta: `当前范围：${scope}`,
      title: "用户",
    };
  }

  if (section === "remotes") {
    return {
      description: "维护可信远端实验室节点，并通过本地节点查询、借用远端公开库存。",
      icon: <ApiOutlined className="settings-placeholder-icon" aria-hidden="true" />,
      meta: `当前范围：${scope}`,
      title: "远端实验室",
    };
  }

  return {
    description: "管理中心已经从设置页迁移到独立 /admin 路由。本轮仅调整后台布局和路由结构。",
    icon: <ApartmentOutlined className="settings-placeholder-icon" aria-hidden="true" />,
    meta: `当前范围：${scope}`,
    title: "管理中心",
  };
}

function RemoteLaboratoriesSection() {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const { message } = AntApp.useApp();
  const [form] = Form.useForm<RemoteSectionFormValues>();
  const remoteLabsQuery = useRemoteLaboratories();
  const createRemoteLab = useCreateRemoteLaboratory();
  const createBorrowRequest = useCreateRemoteBorrowRequest();
  const remotes = remoteLabsQuery.data ?? [];
  const [selectedRemoteId, setSelectedRemoteId] = useState<string | null>(null);
  const remoteInventoryQuery = useRemoteInventory(selectedRemoteId);

  function submitRemote(values: RemoteSectionFormValues) {
    const payload: RemoteLaboratoryPayload = {
      remote_laboratory_id: crypto.randomUUID(),
      name: values.name.trim(),
      api_base_url: values.api_base_url.trim(),
      is_enabled: values.is_enabled,
      key_id: values.key_id.trim(),
      shared_secret: values.shared_secret.trim(),
    };
    createRemoteLab.mutate(payload, {
      onError: (error) => message.error(toMessage(error)),
      onSuccess: (remote) => {
        form.resetFields();
        setSelectedRemoteId(remote.remote_laboratory_id);
        queryClient.invalidateQueries({
          queryKey: adminQueryKeys.remoteLaboratories(apiBaseUrl),
        });
        message.success("远端实验室已保存。");
      },
    });
  }

  function submitBorrow(item: RemoteInventoryItem) {
    if (!selectedRemoteId) {
      return;
    }
    createBorrowRequest.mutate(
      {
        remoteLaboratoryId: selectedRemoteId,
        payload: {
          inventory_item_id: item.inventory_item_id,
          requested_quantity: 1,
          purpose: "demo borrow",
        },
      },
      {
        onError: (error) => message.error(toMessage(error)),
        onSuccess: () => message.success("远端借用申请已提交。"),
      },
    );
  }

  const remoteColumns: TableProps<RemoteLaboratory>["columns"] = [
    {
      title: "名称",
      dataIndex: "name",
      key: "name",
      render: (name: string) => <Text strong>{name}</Text>,
    },
    {
      title: "API 地址",
      dataIndex: "api_base_url",
      key: "api_base_url",
      render: (url: string) => <Text code>{url}</Text>,
    },
    {
      title: "Key ID",
      dataIndex: "key_id",
      key: "key_id",
    },
    {
      title: "状态",
      dataIndex: "is_enabled",
      key: "is_enabled",
      render: (enabled: boolean) => (
        <Tag color={enabled ? "green" : "default"}>{enabled ? "启用" : "停用"}</Tag>
      ),
    },
  ];

  const inventoryColumns: TableProps<RemoteInventoryItem>["columns"] = [
    {
      title: "资产",
      dataIndex: "asset_name",
      key: "asset_name",
      render: (name: string, item) => (
        <Space orientation="vertical" size={0}>
          <Text strong>{name}</Text>
          <Text type="secondary">{item.asset_model ?? "无型号"}</Text>
        </Space>
      ),
    },
    {
      title: "远端实验室",
      dataIndex: "laboratory_name",
      key: "laboratory_name",
    },
    {
      title: "可用数量",
      key: "quantity_available",
      render: (_, item) => `${item.quantity_available} ${item.unit_code}`,
    },
    {
      title: "状态",
      dataIndex: "status",
      key: "status",
    },
    {
      title: "操作",
      key: "actions",
      render: (_, item) => (
        <Button
          type="primary"
          size="small"
          loading={createBorrowRequest.isPending}
          onClick={() => submitBorrow(item)}
        >
          借用 1
        </Button>
      ),
    },
  ];

  return (
    <Space orientation="vertical" size="large" className="full-width">
      <Card className="settings-card" title="登记远端实验室">
        <Form
          form={form}
          layout="vertical"
          initialValues={{ is_enabled: true }}
          onFinish={submitRemote}
        >
          <Form.Item
            label="名称"
            name="name"
            rules={[{ required: true, message: "请输入远端实验室名称。" }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            label="API 地址"
            name="api_base_url"
            rules={[{ required: true, message: "请输入远端 API 地址。" }]}
          >
            <Input placeholder="http://127.0.0.1:8001/api/v1" />
          </Form.Item>
          <Form.Item
            label="Key ID"
            name="key_id"
            rules={[{ required: true, message: "请输入 Key ID。" }]}
          >
            <Input />
          </Form.Item>
          <Form.Item
            label="Shared Secret"
            name="shared_secret"
            rules={[{ required: true, message: "请输入共享密钥。" }]}
          >
            <Input.Password autoComplete="new-password" />
          </Form.Item>
          <Form.Item label="启用" name="is_enabled" valuePropName="checked">
            <Switch />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={createRemoteLab.isPending}>
            保存远端实验室
          </Button>
        </Form>
      </Card>

      <Card className="settings-card" title="远端节点">
        <Table
          columns={remoteColumns}
          dataSource={remotes}
          loading={remoteLabsQuery.isLoading}
          pagination={false}
          rowKey="remote_laboratory_id"
        />
      </Card>

      <Card className="settings-card" title="远端公开库存">
        <Space orientation="vertical" size="middle" className="full-width">
          <Select
            aria-label="选择远端实验室"
            className="full-width"
            placeholder="选择远端实验室"
            value={selectedRemoteId ?? undefined}
            onChange={setSelectedRemoteId}
            options={remotes.map((remote) => ({
              label: remote.name,
              value: remote.remote_laboratory_id,
            }))}
          />
          {remoteInventoryQuery.isError ? (
            <Alert
              showIcon
              type="error"
              title={toMessage(remoteInventoryQuery.error)}
            />
          ) : null}
          <Table
            columns={inventoryColumns}
            dataSource={remoteInventoryQuery.data?.items ?? []}
            loading={remoteInventoryQuery.isFetching}
            pagination={false}
            rowKey="inventory_item_id"
          />
        </Space>
      </Card>
    </Space>
  );
}

function LaboratoriesSection({ currentUser }: { currentUser: CurrentUser }) {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const { message } = AntApp.useApp();
  const laboratoriesQuery = useLaboratories();
  const createLaboratory = useCreateLaboratory();
  const updateLaboratory = useUpdateLaboratory();
  const deleteLaboratory = useDeleteLaboratory();
  const [drawerMode, setDrawerMode] = useState<LaboratoryDrawerMode | null>(null);
  const [activeLaboratory, setActiveLaboratory] = useState<Laboratory | null>(null);
  const [formValues, setFormValues] = useState<LaboratoryFormValues>({
    name: "",
    address: "",
    description: "",
    contact: "",
  });
  const [formErrors, setFormErrors] = useState<LaboratoryFormErrors>({});
  const [deletingLaboratoryId, setDeletingLaboratoryId] = useState<string | null>(null);
  const userTypeName = currentUser.user_type.name;
  const isOwner = userTypeName === "admin";
  const isMaintainer = false;
  const laboratories = laboratoriesQuery.data ?? [];
  const isDrawerOpen = drawerMode !== null;
  const isSaving = createLaboratory.isPending || updateLaboratory.isPending;

  function refreshLaboratories() {
    queryClient.invalidateQueries({
      queryKey: adminQueryKeys.laboratories(apiBaseUrl),
    });
    if (isMaintainer) {
      queryClient.invalidateQueries({ queryKey: ["auth", "me", apiBaseUrl] });
    }
  }

  function openCreateDrawer() {
    setActiveLaboratory(null);
    setFormValues({
      name: "",
      address: "",
      description: "",
      contact: "",
    });
    setFormErrors({});
    setDrawerMode("create");
  }

  function openEditDrawer(laboratory: Laboratory) {
    setActiveLaboratory(laboratory);
    setFormValues({
      name: laboratory.name,
      address: laboratory.address,
      description: laboratory.description ?? "",
      contact: laboratory.contact ?? "",
    });
    setFormErrors({});
    setDrawerMode("edit");
  }

  function closeDrawer() {
    if (isSaving) {
      return;
    }
    setDrawerMode(null);
    setActiveLaboratory(null);
    setFormErrors({});
  }

  function updateFormValue(field: keyof LaboratoryFormValues, value: string) {
    setFormValues((current) => ({
      ...current,
      [field]: value,
    }));
    if (field === "name" || field === "address") {
      setFormErrors((current) => ({
        ...current,
        [field]: undefined,
      }));
    }
  }

  function submitLaboratory(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload = toLaboratoryPayload(formValues);
    const nextErrors: LaboratoryFormErrors = {};
    if (!payload.name) {
      nextErrors.name = "请输入实验室名称。";
    }
    if (!payload.address) {
      nextErrors.address = "请输入地址。";
    }
    setFormErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return;
    }

    if (drawerMode === "create") {
      createLaboratory.mutate(payload, {
        onError: (error) => message.error(toMessage(error)),
        onSuccess: () => {
          closeDrawerAfterSuccess();
          refreshLaboratories();
          message.success("实验室已新增。");
        },
      });
      return;
    }

    if (drawerMode === "edit" && activeLaboratory) {
      updateLaboratory.mutate(
        {
          laboratoryId: activeLaboratory.laboratory_id,
          payload,
        },
        {
          onError: (error) => message.error(toMessage(error)),
          onSuccess: () => {
            closeDrawerAfterSuccess();
            refreshLaboratories();
            message.success("实验室已更新。");
          },
        },
      );
    }
  }

  function closeDrawerAfterSuccess() {
    setDrawerMode(null);
    setActiveLaboratory(null);
    setFormErrors({});
  }

  function confirmDelete(laboratory: Laboratory) {
    setDeletingLaboratoryId(laboratory.laboratory_id);
    deleteLaboratory.mutate(laboratory.laboratory_id, {
      onError: (error) => message.error(toMessage(error)),
      onSettled: () => {
        setDeletingLaboratoryId(null);
      },
      onSuccess: () => {
        refreshLaboratories();
        message.success("实验室已删除。");
      },
    });
  }

  const columns: TableProps<Laboratory>["columns"] = [
    {
      title: "名称",
      dataIndex: "name",
      key: "name",
      render: (name: string) => <Text strong>{name}</Text>,
    },
    {
      title: "地址",
      dataIndex: "address",
      key: "address",
    },
    {
      title: "联系方式",
      dataIndex: "contact",
      key: "contact",
      render: (contact: string | null) => contact ?? <Text type="secondary">未设置</Text>,
    },
    {
      title: "描述",
      dataIndex: "description",
      key: "description",
      render: (description: string | null) =>
        description ?? <Text type="secondary">未设置</Text>,
    },
    {
      title: "操作",
      key: "actions",
      render: (_, laboratory) => (
        <Space wrap>
          <button
            aria-label="编辑"
            className="admin-action-button"
            type="button"
            onClick={() => openEditDrawer(laboratory)}
          >
            <EditOutlined aria-hidden="true" />
            编辑
          </button>
          {isOwner ? (
            <Popconfirm
              title="删除实验室"
              description={`确认删除 ${laboratory.name}？`}
              okText="确认删除"
              cancelText="取消"
              okButtonProps={{
                danger: true,
                loading:
                  deleteLaboratory.isPending &&
                  deletingLaboratoryId === laboratory.laboratory_id,
              }}
              onConfirm={() => confirmDelete(laboratory)}
            >
              <button
                aria-label="删除"
                className="admin-action-button admin-action-button-danger"
                type="button"
                disabled={
                  deleteLaboratory.isPending &&
                  deletingLaboratoryId === laboratory.laboratory_id
                }
              >
                <DeleteOutlined aria-hidden="true" />
                删除
              </button>
            </Popconfirm>
          ) : null}
        </Space>
      ),
    },
  ];

  if (isMaintainer && !currentUser.laboratory) {
    return (
      <Card className="settings-card">
        <Alert
          showIcon
          type="warning"
          title="当前账号未绑定实验室。"
          description="请联系系统所有者为该账号绑定实验室后再进行管理。"
        />
      </Card>
    );
  }

  return (
    <>
      <Card
        className="settings-card"
        title="实验室列表"
        extra={
          isOwner ? (
            <Button
              aria-label="新增实验室"
              type="primary"
              icon={<PlusOutlined aria-hidden="true" />}
              onClick={openCreateDrawer}
            >
              新增实验室
            </Button>
          ) : null
        }
      >
        {laboratoriesQuery.isError ? (
          <Alert showIcon type="error" title={toMessage(laboratoriesQuery.error)} />
        ) : null}
        <Table<Laboratory>
          rowKey="laboratory_id"
          columns={columns}
          dataSource={laboratories}
          loading={laboratoriesQuery.isLoading}
          pagination={false}
          scroll={{ x: 760 }}
        />
      </Card>

      <Drawer
        destroyOnHidden
        forceRender
        title={drawerMode === "create" ? "新增实验室" : "编辑实验室"}
        open={isDrawerOpen}
        onClose={closeDrawer}
        size="large"
        footer={
          <div className="admin-drawer-footer">
            <Space wrap>
              <Button onClick={closeDrawer} disabled={isSaving}>
                取消
              </Button>
              <Button
                type="primary"
                htmlType="submit"
                form="admin-laboratory-form"
                loading={isSaving}
              >
                保存
              </Button>
            </Space>
          </div>
        }
      >
        <Form
          aria-label="实验室表单"
          id="admin-laboratory-form"
          layout="vertical"
          requiredMark={false}
          onSubmitCapture={submitLaboratory}
        >
          <Form.Item
            htmlFor="laboratory-name"
            label="实验室名称"
            required
            validateStatus={formErrors.name ? "error" : undefined}
            help={formErrors.name}
          >
            <Input
              id="laboratory-name"
              autoComplete="off"
              value={formValues.name}
              onChange={(event) => updateFormValue("name", event.target.value)}
            />
          </Form.Item>
          <Form.Item
            htmlFor="laboratory-address"
            label="地址"
            required
            validateStatus={formErrors.address ? "error" : undefined}
            help={formErrors.address}
          >
            <Input
              id="laboratory-address"
              autoComplete="off"
              value={formValues.address}
              onChange={(event) => updateFormValue("address", event.target.value)}
            />
          </Form.Item>
          <Form.Item htmlFor="laboratory-description" label="描述">
            <Input.TextArea
              id="laboratory-description"
              autoSize={{ minRows: 3, maxRows: 6 }}
              value={formValues.description}
              onChange={(event) => updateFormValue("description", event.target.value)}
            />
          </Form.Item>
          <Form.Item htmlFor="laboratory-contact" label="联系方式">
            <Input
              id="laboratory-contact"
              autoComplete="off"
              value={formValues.contact}
              onChange={(event) => updateFormValue("contact", event.target.value)}
            />
          </Form.Item>
        </Form>
      </Drawer>
    </>
  );
}

function UsersSection({ currentUser }: { currentUser: CurrentUser }) {
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const { message } = AntApp.useApp();
  const usersQuery = useUsers();
  const userTypeName = currentUser.user_type.name;
  const isOwner = userTypeName === "admin";
  const isMaintainer = false;
  const laboratoriesQuery = useLaboratories({ enabled: isOwner });
  const createUser = useCreateUser();
  const updateUser = useUpdateUser();
  const deleteUser = useDeleteUser();
  const [drawerMode, setDrawerMode] = useState<UserDrawerMode | null>(null);
  const [activeUser, setActiveUser] = useState<AdminUser | null>(null);
  const [formValues, setFormValues] = useState<UserFormValues>({
    username: "",
    email: "",
    password: "",
    user_type: "user",
    laboratory_id: "",
  });
  const [formErrors, setFormErrors] = useState<UserFormErrors>({});
  const [deletingUserId, setDeletingUserId] = useState<string | null>(null);
  const users = usersQuery.data ?? [];
  const laboratories = laboratoriesQuery.data ?? [];
  const isDrawerOpen = drawerMode !== null;
  const isSaving = createUser.isPending || updateUser.isPending;
  const isActiveCurrentUser = activeUser?.user_id === currentUser.user_id;
  const roleOptions = getRoleOptions(userTypeName);

  function refreshUsers(targetUserId?: string) {
    queryClient.invalidateQueries({
      queryKey: adminQueryKeys.users(apiBaseUrl),
    });
    if (targetUserId === currentUser.user_id) {
      queryClient.invalidateQueries({ queryKey: ["auth", "me", apiBaseUrl] });
    }
  }

  function defaultLaboratoryId() {
    if (isMaintainer) {
      return currentUser.laboratory?.laboratory_id ?? "";
    }
    return laboratories[0]?.laboratory_id ?? "";
  }

  function openCreateDrawer() {
    setActiveUser(null);
    setFormValues({
      username: "",
      email: "",
      password: "",
      user_type: "user",
      laboratory_id: defaultLaboratoryId(),
    });
    setFormErrors({});
    setDrawerMode("create");
  }

  function openEditDrawer(user: AdminUser) {
    setActiveUser(user);
    setFormValues({
      username: user.username,
      email: user.email ?? "",
      password: "",
      user_type: user.user_type.name,
      laboratory_id: user.laboratory?.laboratory_id ?? defaultLaboratoryId(),
    });
    setFormErrors({});
    setDrawerMode("edit");
  }

  function closeDrawer() {
    if (isSaving) {
      return;
    }
    setDrawerMode(null);
    setActiveUser(null);
    setFormErrors({});
  }

  function closeDrawerAfterSuccess() {
    setDrawerMode(null);
    setActiveUser(null);
    setFormErrors({});
  }

  function updateFormValue(field: keyof UserFormValues, value: string) {
    setFormValues((current) => {
      if (field === "user_type") {
        return {
          ...current,
          user_type: value,
          laboratory_id: requiresLaboratory(value)
            ? current.laboratory_id || defaultLaboratoryId()
            : "",
        };
      }
      return {
        ...current,
        [field]: value,
      };
    });
    if (field === "username" || field === "password" || field === "laboratory_id") {
      setFormErrors((current) => ({
        ...current,
        [field]: undefined,
      }));
    }
  }

  function submitUser(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextErrors: UserFormErrors = {};
    const username = formValues.username.trim();
    const password = formValues.password.trim();
    const roleName = formValues.user_type;
    const laboratoryId = resolveUserFormLaboratory(roleName);

    if (!username) {
      nextErrors.username = "请输入用户名。";
    }
    if (drawerMode === "create" && !password) {
      nextErrors.password = "请输入初始密码。";
    }
    if (drawerMode === "edit" && formValues.password.length > 0 && !password) {
      nextErrors.password = "密码不能为空。";
    }
    if (!isActiveCurrentUser && requiresLaboratory(roleName) && !laboratoryId) {
      nextErrors.laboratory_id = "请选择实验室。";
    }
    setFormErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return;
    }

    if (drawerMode === "create") {
      const payload: CreateUserPayload = {
        email: optionalText(formValues.email),
        laboratory_id: laboratoryId,
        password,
        user_type: roleName,
        username,
      };
      createUser.mutate(payload, {
        onError: (error) => message.error(toMessage(error)),
        onSuccess: () => {
          closeDrawerAfterSuccess();
          refreshUsers();
          message.success("用户已新增。");
        },
      });
      return;
    }

    if (drawerMode === "edit" && activeUser) {
      const payload: UpdateUserPayload = {
        email: optionalText(formValues.email),
        username,
      };
      if (password) {
        payload.password = password;
      }
      if (!isActiveCurrentUser) {
        payload.user_type = roleName;
        payload.laboratory_id = laboratoryId;
      }
      updateUser.mutate(
        {
          payload,
          userId: activeUser.user_id,
        },
        {
          onError: (error) => message.error(toMessage(error)),
          onSuccess: () => {
            closeDrawerAfterSuccess();
            refreshUsers(activeUser.user_id);
            message.success("用户已更新。");
          },
        },
      );
    }
  }

  function resolveUserFormLaboratory(roleName: string) {
    if (!requiresLaboratory(roleName)) {
      return null;
    }
    if (isMaintainer) {
      return currentUser.laboratory?.laboratory_id ?? null;
    }
    return formValues.laboratory_id || null;
  }

  function confirmDelete(user: AdminUser) {
    setDeletingUserId(user.user_id);
    deleteUser.mutate(user.user_id, {
      onError: (error) => message.error(toMessage(error)),
      onSettled: () => {
        setDeletingUserId(null);
      },
      onSuccess: () => {
        refreshUsers();
        message.success("用户已删除。");
      },
    });
  }

  function canEditUser(user: AdminUser) {
    if (isOwner) {
      return true;
    }
    return isMaintainer && user.laboratory?.laboratory_id === currentUser.laboratory?.laboratory_id;
  }

  function canDeleteUser(user: AdminUser) {
    return user.user_id !== currentUser.user_id && canEditUser(user);
  }

  const columns: TableProps<AdminUser>["columns"] = [
    {
      title: "用户名",
      dataIndex: "username",
      key: "username",
      render: (username: string) => <Text strong>{username}</Text>,
    },
    {
      title: "邮箱",
      dataIndex: "email",
      key: "email",
      render: (email: string | null) => email ?? <Text type="secondary">未设置</Text>,
    },
    {
      title: "角色",
      dataIndex: ["user_type", "name"],
      key: "user_type",
      render: (roleName: string) => (
        <Tag color={getRoleColor(roleName)}>{getRoleLabel(roleName)}</Tag>
      ),
    },
    {
      title: "实验室",
      dataIndex: "laboratory",
      key: "laboratory",
      render: (laboratory: AdminUser["laboratory"]) =>
        laboratory?.name ?? <Text type="secondary">全部实验室</Text>,
    },
    {
      title: "创建时间",
      dataIndex: "created_at",
      key: "created_at",
      render: (createdAt: string) => formatDate(createdAt),
    },
    {
      title: "最近登录",
      dataIndex: "last_login_at",
      key: "last_login_at",
      render: (lastLoginAt: string | null) =>
        lastLoginAt ? formatDate(lastLoginAt) : <Text type="secondary">未登录</Text>,
    },
    {
      title: "操作",
      key: "actions",
      render: (_, user) => (
        <Space wrap>
          <button
            aria-label="编辑"
            className="admin-action-button"
            type="button"
            disabled={!canEditUser(user)}
            onClick={() => openEditDrawer(user)}
          >
            <EditOutlined aria-hidden="true" />
            编辑
          </button>
          {canDeleteUser(user) ? (
            <Popconfirm
              title="删除用户"
              description={`确认删除 ${user.username}？`}
              okText="确认删除"
              cancelText="取消"
              okButtonProps={{
                danger: true,
                loading: deleteUser.isPending && deletingUserId === user.user_id,
              }}
              onConfirm={() => confirmDelete(user)}
            >
              <button
                aria-label="删除"
                className="admin-action-button admin-action-button-danger"
                type="button"
                disabled={deleteUser.isPending && deletingUserId === user.user_id}
              >
                <DeleteOutlined aria-hidden="true" />
                删除
              </button>
            </Popconfirm>
          ) : (
            <button
              aria-label="删除"
              className="admin-action-button admin-action-button-danger"
              type="button"
              disabled
            >
              <DeleteOutlined aria-hidden="true" />
              删除
            </button>
          )}
        </Space>
      ),
    },
  ];

  if (isMaintainer && !currentUser.laboratory) {
    return (
      <Card className="settings-card">
        <Alert
          showIcon
          type="warning"
          title="当前账号未绑定实验室。"
          description="请联系系统所有者为该账号绑定实验室后再进行管理。"
        />
      </Card>
    );
  }

  return (
    <>
      <Card
        className="settings-card"
        title="用户列表"
        extra={
          <Button
            aria-label="新增用户"
            type="primary"
            icon={<PlusOutlined aria-hidden="true" />}
            onClick={openCreateDrawer}
          >
            新增用户
          </Button>
        }
      >
        {usersQuery.isError ? (
          <Alert showIcon type="error" title={toMessage(usersQuery.error)} />
        ) : null}
        {isOwner && laboratoriesQuery.isError ? (
          <Alert showIcon type="error" title={toMessage(laboratoriesQuery.error)} />
        ) : null}
        <Table<AdminUser>
          rowKey="user_id"
          columns={columns}
          dataSource={users}
          loading={usersQuery.isLoading || (isOwner && laboratoriesQuery.isLoading)}
          pagination={false}
          scroll={{ x: 980 }}
        />
      </Card>

      <Drawer
        destroyOnHidden
        forceRender
        title={drawerMode === "create" ? "新增用户" : "编辑用户"}
        open={isDrawerOpen}
        onClose={closeDrawer}
        size="large"
        footer={
          <div className="admin-drawer-footer">
            <Space wrap>
              <Button onClick={closeDrawer} disabled={isSaving}>
                取消
              </Button>
              <Button
                type="primary"
                htmlType="submit"
                form="admin-user-form"
                loading={isSaving}
              >
                保存
              </Button>
            </Space>
          </div>
        }
      >
        <Form
          aria-label="用户表单"
          id="admin-user-form"
          layout="vertical"
          requiredMark={false}
          onSubmitCapture={submitUser}
        >
          <Form.Item
            htmlFor="user-username"
            label="用户名"
            required
            validateStatus={formErrors.username ? "error" : undefined}
            help={formErrors.username}
          >
            <Input
              id="user-username"
              autoComplete="off"
              value={formValues.username}
              onChange={(event) => updateFormValue("username", event.target.value)}
            />
          </Form.Item>
          <Form.Item htmlFor="user-email" label="邮箱">
            <Input
              id="user-email"
              autoComplete="off"
              value={formValues.email}
              onChange={(event) => updateFormValue("email", event.target.value)}
            />
          </Form.Item>
          <Form.Item
            htmlFor="user-password"
            label="密码"
            required={drawerMode === "create"}
            validateStatus={formErrors.password ? "error" : undefined}
            help={
              formErrors.password ??
              (drawerMode === "edit" ? "留空则不修改密码。" : undefined)
            }
          >
            <Input.Password
              id="user-password"
              autoComplete="new-password"
              value={formValues.password}
              onChange={(event) => updateFormValue("password", event.target.value)}
            />
          </Form.Item>
          <Form.Item label="用户类型">
            <Radio.Group
              aria-label="用户类型"
              value={formValues.user_type}
              disabled={isActiveCurrentUser}
              onChange={(event) => updateFormValue("user_type", event.target.value)}
              options={roleOptions.map((role) => ({
                label: getRoleLabel(role),
                value: role,
              }))}
            />
          </Form.Item>
          {isOwner && requiresLaboratory(formValues.user_type) ? (
            <Form.Item
              htmlFor="user-laboratory"
              label="实验室"
              required
              validateStatus={formErrors.laboratory_id ? "error" : undefined}
              help={formErrors.laboratory_id}
            >
              <Select
                id="user-laboratory"
                aria-label="实验室"
                loading={laboratoriesQuery.isLoading}
                value={formValues.laboratory_id || undefined}
                disabled={isActiveCurrentUser}
                placeholder="选择实验室"
                onChange={(value) => updateFormValue("laboratory_id", value)}
                options={laboratories.map((laboratory) => ({
                  label: laboratory.name,
                  value: laboratory.laboratory_id,
                }))}
              />
            </Form.Item>
          ) : null}
        </Form>
      </Drawer>
    </>
  );
}

function toLaboratoryPayload(values: LaboratoryFormValues): LaboratoryPayload {
  return {
    name: values.name.trim(),
    address: values.address.trim(),
    description: optionalText(values.description),
    contact: optionalText(values.contact),
  };
}

function getRoleOptions(currentUserType: string) {
  if (currentUserType === "admin") {
    return ["admin", "user"];
  }
  return ["user"];
}

function getRoleLabel(roleName: string) {
  if (roleName === "admin") {
    return "管理员";
  }
  if (roleName === "user") {
    return "用户";
  }
  return roleName;
}

function getRoleColor(roleName: string) {
  if (roleName === "admin") {
    return "red";
  }
  if (roleName === "user") {
    return "green";
  }
  return "default";
}

function requiresLaboratory(roleName: string) {
  return ["admin", "user"].includes(roleName);
}

function formatDate(value: string) {
  return value.slice(0, 10);
}

function optionalText(value: string | undefined) {
  const normalized = value?.trim() ?? "";
  return normalized.length > 0 ? normalized : null;
}

function AdminAccessDenied() {
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

function toMessage(caught: unknown) {
  if (caught instanceof Error) {
    return caught.message;
  }
  return "操作失败。";
}
