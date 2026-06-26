import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useQueryClient } from "@tanstack/react-query";
import {
  ChevronDown,
  ChevronRight,
  FolderTree,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useAuth } from "../../app/auth-context";
import { useLaboratorySelection } from "../../app/laboratory-selection-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { formatDate } from "../../shared/lib/date";
import { toErrorMessage } from "../../shared/lib/errors";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { ConfirmDialog } from "../../shared/ui/ConfirmDialog";
import { Dialog } from "../../shared/ui/Dialog";
import { EmptyState } from "../../shared/ui/EmptyState";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { Select } from "../../shared/ui/Select";
import { useToast } from "../../shared/ui/Toast";
import { canManageAssetCategories } from "../auth/permissions";
import {
  adminQueryKeys,
  type AssetCategory,
  type AssetCategoryParameterAssignmentPayload,
  type AssetCategoryPayload,
  type AssetParameter,
  optionalText,
  useAssetCategories,
  useAssetParameters,
  useCreateAssetCategory,
  useDeleteAssetCategory,
  useUpdateAssetCategory,
} from "./api";

type CategoryEditorState =
  | {
      mode: "create";
      parentCategoryId: string | null;
    }
  | {
      category: AssetCategory;
      mode: "edit";
    };

type CategoryForm = {
  code: string;
  description: string;
  name: string;
  parameter_assignments: CategoryParameterAssignmentForm[];
  parent_category_id: string;
};

type CategoryParameterAssignmentForm = {
  applies_to_descendants: boolean;
  assignment_id?: string;
  client_id: string;
  is_required: boolean;
  parameter_type_id: string;
  sort_order: string;
};

type CategoryTreeRow = {
  category: AssetCategory;
  depth: number;
};

const EMPTY_CATEGORIES: AssetCategory[] = [];
const EMPTY_PARAMETERS: AssetParameter[] = [];

export function AssetCategoriesPage() {
  const { currentUser } = useAuth();
  const {
    canManageSelectedLaboratoryAssets,
    selectedLaboratoryId,
    selectedLaboratoryName,
  } = useLaboratorySelection();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const canManage = canManageAssetCategories(currentUser);
  const categoriesQuery = useAssetCategories({
    enabled: canManageSelectedLaboratoryAssets && Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const parametersQuery = useAssetParameters({
    enabled: canManageSelectedLaboratoryAssets && Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const createCategory = useCreateAssetCategory();
  const updateCategory = useUpdateAssetCategory();
  const deleteCategory = useDeleteAssetCategory();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
  const [editing, setEditing] = useState<CategoryEditorState | null>(null);
  const categories = categoriesQuery.data ?? EMPTY_CATEGORIES;
  const parameters = parametersQuery.data ?? EMPTY_PARAMETERS;
  const childrenByParentId = useMemo(() => buildCategoryChildren(categories), [categories]);
  const visibleRows = useMemo(
    () => flattenVisibleCategories(childrenByParentId, expandedIds),
    [childrenByParentId, expandedIds],
  );

  useEffect(() => {
    setExpandedIds(new Set());
  }, [selectedLaboratoryId]);

  function refresh() {
    if (!selectedLaboratoryId) return;
    queryClient.invalidateQueries({
      queryKey: adminQueryKeys.assetCategories(apiBaseUrl, selectedLaboratoryId),
    });
  }

  function toggleCategory(categoryId: string) {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(categoryId)) {
        next.delete(categoryId);
      } else {
        next.add(categoryId);
      }
      return next;
    });
  }

  function openCreate(parentCategoryId: string | null = null) {
    setEditing({ mode: "create", parentCategoryId });
  }

  function handleDelete(category: AssetCategory) {
    deleteCategory.mutate(category.category_id, {
      onError: (error) =>
        toast.error({ title: "删除资产分类失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        refresh();
        toast.success({ title: "资产分类已删除" });
      },
    });
  }

  const pageActions = (
    <Button
      disabled={!canManageSelectedLaboratoryAssets || !selectedLaboratoryId}
      onClick={() => openCreate()}
      variant="primary"
    >
      <Plus size={15} />
      新建分类
    </Button>
  );

  if (!canManage) {
    return (
      <main className="page">
        <PageHeader
          kicker="管理"
          title="资产分类"
          description="当前账号没有管理资产分类的权限。"
        />
        <section className="panel">
          <EmptyState description="请切换到有权限的账号。" title="权限不足" />
        </section>
      </main>
    );
  }

  return (
    <main className="page">
      <PageHeader
        kicker="管理"
        title="资产分类"
        description="按实验室维护资产分类树，分类代码会参与生成层级路径。"
        actions={pageActions}
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">{selectedLaboratoryName || "未选择实验室"}</h2>
            <p className="panel-description">
              {!selectedLaboratoryId
                ? "请选择实验室后管理分类"
                : canManageSelectedLaboratoryAssets
                  ? `${categories.length} 个分类`
                  : "当前账号不能管理该实验室"}
            </p>
          </div>
        </div>
        <CategoryTreeTable
          categories={categories}
          childrenByParentId={childrenByParentId}
          deleting={deleteCategory.isPending}
          expandedIds={expandedIds}
          loading={categoriesQuery.isLoading}
          rows={visibleRows}
          onCreateChild={(category) => openCreate(category.category_id)}
          onDelete={handleDelete}
          onEdit={(category) => setEditing({ category, mode: "edit" })}
          onToggle={toggleCategory}
        />
      </section>
      <CategoryEditor
        categories={categories}
        createCategory={createCategory}
        editing={editing}
        laboratoryId={selectedLaboratoryId}
        open={editing !== null}
        parameters={parameters}
        parametersLoading={parametersQuery.isLoading}
        updateCategory={updateCategory}
        onClose={() => setEditing(null)}
        onSaved={(parentCategoryId) => {
          setEditing(null);
          if (parentCategoryId) {
            setExpandedIds((current) => new Set(current).add(parentCategoryId));
          }
          refresh();
        }}
      />
    </main>
  );
}

function CategoryTreeTable({
  categories,
  childrenByParentId,
  deleting,
  expandedIds,
  loading,
  onCreateChild,
  onDelete,
  onEdit,
  onToggle,
  rows,
}: {
  categories: AssetCategory[];
  childrenByParentId: Map<string | null, AssetCategory[]>;
  deleting: boolean;
  expandedIds: Set<string>;
  loading: boolean;
  onCreateChild: (category: AssetCategory) => void;
  onDelete: (category: AssetCategory) => void;
  onEdit: (category: AssetCategory) => void;
  onToggle: (categoryId: string) => void;
  rows: CategoryTreeRow[];
}) {
  if (loading) {
    return (
      <div className="panel-body">
        <div className="skeleton" style={{ height: 220 }} />
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <EmptyState
        description="当前实验室还没有资产分类。"
        title="暂无资产分类"
      />
    );
  }

  return (
    <div className="table-wrap">
      <table className="data-table category-tree-table">
        <thead>
          <tr>
            <th>分类</th>
            <th>代码</th>
            <th>参数</th>
            <th>描述</th>
            <th>更新时间</th>
            <th style={{ textAlign: "right" }}>操作</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ category, depth }) => {
            const children = childrenByParentId.get(category.category_id) ?? [];
            const hasChildren = children.length > 0;
            const expanded = expandedIds.has(category.category_id);
            const descendantCount = countDescendants(categories, category);

            return (
              <tr key={category.category_id}>
                <td>
                  <div
                    className="category-tree-cell"
                    style={{ paddingLeft: depth * 24 }}
                  >
                    <Button
                      aria-label={expanded ? "收起分类" : "展开分类"}
                      className="category-tree-toggle"
                      disabled={!hasChildren}
                      size="icon"
                      variant="ghost"
                      onClick={() => onToggle(category.category_id)}
                    >
                      {hasChildren ? (
                        expanded ? (
                          <ChevronDown size={14} />
                        ) : (
                          <ChevronRight size={14} />
                        )
                      ) : (
                        <span className="category-tree-toggle-placeholder" />
                      )}
                    </Button>
                    <FolderTree size={15} aria-hidden="true" />
                    <div className="category-tree-title">
                      <strong>{category.name}</strong>
                      <span>{category.path}</span>
                    </div>
                  </div>
                </td>
                <td>
                  <Badge>{category.code}</Badge>
                </td>
                <td>
                  <Badge tone={category.parameter_assignments.length > 0 ? "accent" : "default"}>
                    {category.parameter_assignments.length} 个
                  </Badge>
                </td>
                <td>{category.description ?? "未填写"}</td>
                <td>{formatDate(category.updated_at)}</td>
                <td style={{ textAlign: "right" }}>
                  <span className="table-actions">
                    <Button
                      aria-label="新增子分类"
                      size="icon"
                      variant="ghost"
                      onClick={() => onCreateChild(category)}
                    >
                      <Plus size={15} />
                    </Button>
                    <Button
                      aria-label="编辑"
                      size="icon"
                      variant="ghost"
                      onClick={() => onEdit(category)}
                    >
                      <Pencil size={15} />
                    </Button>
                    <ConfirmDialog
                      confirmLabel="删除"
                      description={
                        descendantCount > 0
                          ? `删除「${category.name}」会同时删除 ${descendantCount} 个子分类。确认继续？`
                          : `删除「${category.name}」会同时删除所有子分类。当前没有子分类，确认继续？`
                      }
                      disabled={deleting}
                      title="删除资产分类"
                      trigger={
                        <Button size="icon" variant="ghost" aria-label="删除">
                          <Trash2 size={15} />
                        </Button>
                      }
                      onConfirm={() => onDelete(category)}
                    />
                  </span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function CategoryEditor({
  categories,
  createCategory,
  editing,
  laboratoryId,
  onClose,
  onSaved,
  open,
  parameters,
  parametersLoading,
  updateCategory,
}: {
  categories: AssetCategory[];
  createCategory: ReturnType<typeof useCreateAssetCategory>;
  editing: CategoryEditorState | null;
  laboratoryId: string;
  onClose: () => void;
  onSaved: (parentCategoryId: string | null) => void;
  open: boolean;
  parameters: AssetParameter[];
  parametersLoading: boolean;
  updateCategory: ReturnType<typeof useUpdateAssetCategory>;
}) {
  const toast = useToast();
  const isNew = editing?.mode === "create";
  const editingCategory = editing?.mode === "edit" ? editing.category : null;
  const [values, setValues] = useState<CategoryForm>(emptyCategoryForm());
  const parentOptions = useMemo(
    () => filterParentOptions(categories, editingCategory),
    [categories, editingCategory],
  );
  const isSaving = createCategory.isPending || updateCategory.isPending;

  useEffect(() => {
    if (!editing) {
      setValues(emptyCategoryForm());
      return;
    }

    if (editing.mode === "create") {
      setValues(emptyCategoryForm(editing.parentCategoryId ?? ""));
      return;
    }

    setValues({
      code: editing.category.code,
      description: editing.category.description ?? "",
      name: editing.category.name,
      parameter_assignments: editing.category.parameter_assignments.map((assignment) => ({
        applies_to_descendants: assignment.applies_to_descendants,
        assignment_id: assignment.assignment_id,
        client_id: assignment.assignment_id,
        is_required: assignment.is_required,
        parameter_type_id: assignment.parameter_type_id,
        sort_order: String(assignment.sort_order),
      })),
      parent_category_id: editing.category.parent_category_id ?? "",
    });
  }, [editing]);

  function updateField<K extends keyof Omit<CategoryForm, "parameter_assignments">>(
    field: K,
    value: CategoryForm[K],
  ) {
    setValues((current) => ({ ...current, [field]: value }));
  }

  function addParameterAssignment() {
    const selectedParameterIds = new Set(
      values.parameter_assignments
        .map((assignment) => assignment.parameter_type_id)
        .filter(Boolean),
    );
    const nextParameter = parameters.find(
      (parameter) =>
        !selectedParameterIds.has(parameter.parameter_type_id),
    );

    if (!nextParameter) {
      toast.error({ title: "没有可添加的启用参数" });
      return;
    }

    setValues((current) => ({
      ...current,
      parameter_assignments: [
        ...current.parameter_assignments,
        emptyParameterAssignmentForm(nextParameter.parameter_type_id),
      ],
    }));
  }

  function updateParameterAssignment<K extends keyof CategoryParameterAssignmentForm>(
    index: number,
    field: K,
    value: CategoryParameterAssignmentForm[K],
  ) {
    setValues((current) => ({
      ...current,
      parameter_assignments: current.parameter_assignments.map((assignment, assignmentIndex) =>
        assignmentIndex === index ? { ...assignment, [field]: value } : assignment,
      ),
    }));
  }

  function removeParameterAssignment(index: number) {
    setValues((current) => ({
      ...current,
      parameter_assignments: current.parameter_assignments.filter(
        (_, assignmentIndex) => assignmentIndex !== index,
      ),
    }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!laboratoryId) {
      toast.error({ title: "请先选择实验室" });
      return;
    }

    const payload: AssetCategoryPayload = {
      code: values.code.trim(),
      description: optionalText(values.description),
      name: values.name.trim(),
      parameter_assignments: [],
      parent_category_id: values.parent_category_id || null,
    };

    if (!payload.name || !payload.code) {
      toast.error({ title: "请填写分类名和分类代码" });
      return;
    }

    const assignmentsResult = normalizeParameterAssignments(values.parameter_assignments);
    if (!assignmentsResult.ok) {
      toast.error({ title: assignmentsResult.message });
      return;
    }
    payload.parameter_assignments = assignmentsResult.parameter_assignments;

    if (isNew) {
      createCategory.mutate(
        { laboratoryId, payload },
        {
          onError: (error) =>
            toast.error({ title: "创建资产分类失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "资产分类已创建" });
            onSaved(payload.parent_category_id);
          },
        },
      );
      return;
    }

    if (editingCategory) {
      updateCategory.mutate(
        { categoryId: editingCategory.category_id, payload },
        {
          onError: (error) =>
            toast.error({ title: "更新资产分类失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "资产分类已更新" });
            onSaved(payload.parent_category_id);
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="分类代码只能在同一上级分类下保持唯一，保存后会刷新分类树。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "新建资产分类" : "编辑资产分类"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="asset-category-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="asset-category-form" onSubmit={handleSubmit}>
        <FormField htmlFor="asset-category-parent" label="上级分类">
          <CategoryParentPicker
            categories={parentOptions}
            id="asset-category-parent"
            value={values.parent_category_id}
            onValueChange={(value) => updateField("parent_category_id", value)}
          />
        </FormField>
        <FormField htmlFor="asset-category-name" label="分类名">
          <input
            className="input"
            id="asset-category-name"
            value={values.name}
            onChange={(event) => updateField("name", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="asset-category-code" label="分类代码">
          <input
            className="input"
            id="asset-category-code"
            value={values.code}
            onChange={(event) => updateField("code", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="asset-category-description" label="描述">
          <textarea
            className="textarea"
            id="asset-category-description"
            value={values.description}
            onChange={(event) => updateField("description", event.target.value)}
          />
        </FormField>
        <CategoryParameterAssignmentsEditor
          assignments={values.parameter_assignments}
          loading={parametersLoading}
          parameters={parameters}
          onAdd={addParameterAssignment}
          onRemove={removeParameterAssignment}
          onUpdate={updateParameterAssignment}
        />
      </form>
    </Dialog>
  );
}

function CategoryParameterAssignmentsEditor({
  assignments,
  loading,
  onAdd,
  onRemove,
  onUpdate,
  parameters,
}: {
  assignments: CategoryParameterAssignmentForm[];
  loading: boolean;
  onAdd: () => void;
  onRemove: (index: number) => void;
  onUpdate: <K extends keyof CategoryParameterAssignmentForm>(
    index: number,
    field: K,
    value: CategoryParameterAssignmentForm[K],
  ) => void;
  parameters: AssetParameter[];
}) {
  const selectedParameterIds = new Set(
    assignments.map((assignment) => assignment.parameter_type_id).filter(Boolean),
  );

  return (
    <div className="category-parameter-assignments-field">
      <div className="parameter-options-header">
        <div>
          <h3>附带参数</h3>
          <p>选择该分类资产需要维护的参数。</p>
        </div>
        <Button disabled={loading} type="button" onClick={onAdd}>
          <Plus size={15} />
          添加参数
        </Button>
      </div>
      {assignments.length === 0 ? (
        <div className="category-parameter-empty">未绑定参数</div>
      ) : (
        <div className="category-parameter-assignments-list">
          {assignments.map((assignment, index) => {
            const availableParameters = parameters.filter(
              (parameter) =>
                parameter.parameter_type_id === assignment.parameter_type_id ||
                (!selectedParameterIds.has(parameter.parameter_type_id)),
            );

            return (
              <div className="category-parameter-assignment-row" key={assignment.client_id}>
                <FormField
                  htmlFor={`asset-category-parameter-${assignment.client_id}`}
                  label="参数"
                >
                  <select
                    className="input"
                    id={`asset-category-parameter-${assignment.client_id}`}
                    value={assignment.parameter_type_id}
                    onChange={(event) =>
                      onUpdate(index, "parameter_type_id", event.target.value)
                    }
                  >
                    <option value="">请选择参数</option>
                    {availableParameters.map((parameter) => (
                      <option
                        key={parameter.parameter_type_id}
                        value={parameter.parameter_type_id}
                      >
                        {parameter.name}（{parameter.code}）
                      </option>
                    ))}
                  </select>
                </FormField>
                <label
                  className="checkbox-field category-parameter-assignment-toggle"
                  htmlFor={`asset-category-parameter-required-${assignment.client_id}`}
                >
                  <input
                    checked={assignment.is_required}
                    id={`asset-category-parameter-required-${assignment.client_id}`}
                    type="checkbox"
                    onChange={(event) =>
                      onUpdate(index, "is_required", event.target.checked)
                    }
                  />
                  <span>
                    <strong>必填</strong>
                  </span>
                </label>
                <label
                  className="checkbox-field category-parameter-assignment-toggle"
                  htmlFor={`asset-category-parameter-descendants-${assignment.client_id}`}
                >
                  <input
                    checked={assignment.applies_to_descendants}
                    id={`asset-category-parameter-descendants-${assignment.client_id}`}
                    type="checkbox"
                    onChange={(event) =>
                      onUpdate(index, "applies_to_descendants", event.target.checked)
                    }
                  />
                  <span>
                    <strong>适用于子分类</strong>
                  </span>
                </label>
                <FormField
                  htmlFor={`asset-category-parameter-sort-${assignment.client_id}`}
                  label="排序"
                >
                  <input
                    className="input"
                    id={`asset-category-parameter-sort-${assignment.client_id}`}
                    type="number"
                    value={assignment.sort_order}
                    onChange={(event) => onUpdate(index, "sort_order", event.target.value)}
                  />
                </FormField>
                <Button
                  aria-label="删除附带参数"
                  className="category-parameter-assignment-delete"
                  size="icon"
                  type="button"
                  variant="ghost"
                  onClick={() => onRemove(index)}
                >
                  <Trash2 size={15} />
                </Button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function CategoryParentPicker({
  categories,
  id,
  onValueChange,
  value,
}: {
  categories: AssetCategory[];
  id: string;
  onValueChange: (value: string) => void;
  value: string;
}) {
  const childrenByParentId = useMemo(() => buildCategoryChildren(categories), [categories]);
  const selectedCategory = categories.find((category) => category.category_id === value);
  const rootCategories = childrenByParentId.get(null) ?? [];

  function renderCategoryOption(category: AssetCategory): ReactNode {
    const children = childrenByParentId.get(category.category_id) ?? [];
    const label = (
      <span className="category-parent-option">
        <span>{category.name}</span>
        <span>{category.code}</span>
      </span>
    );

    if (children.length === 0) {
      return (
        <DropdownMenu.Item
          className="dropdown-item"
          key={category.category_id}
          onSelect={() => onValueChange(category.category_id)}
        >
          {label}
        </DropdownMenu.Item>
      );
    }

    return (
      <DropdownMenu.Sub key={category.category_id}>
        <DropdownMenu.SubTrigger className="dropdown-item category-parent-subtrigger">
          {label}
          <ChevronRight size={13} aria-hidden="true" />
        </DropdownMenu.SubTrigger>
        <DropdownMenu.Portal>
          <DropdownMenu.SubContent className="dropdown-content category-parent-menu">
            <DropdownMenu.Item
              className="dropdown-item"
              onSelect={() => onValueChange(category.category_id)}
            >
              选择“{category.name}”
            </DropdownMenu.Item>
            <DropdownMenu.Separator className="dropdown-separator" />
            {children.map(renderCategoryOption)}
          </DropdownMenu.SubContent>
        </DropdownMenu.Portal>
      </DropdownMenu.Sub>
    );
  }

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button className="category-parent-trigger" id={id}>
          <span>{selectedCategory ? selectedCategory.name : "无上级分类"}</span>
          <ChevronDown size={14} aria-hidden="true" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="dropdown-content category-parent-menu" align="start">
          <DropdownMenu.Item className="dropdown-item" onSelect={() => onValueChange("")}>
            无上级分类
          </DropdownMenu.Item>
          {rootCategories.length > 0 ? (
            <>
              <DropdownMenu.Separator className="dropdown-separator" />
              {rootCategories.map(renderCategoryOption)}
            </>
          ) : null}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function buildCategoryChildren(categories: AssetCategory[]) {
  const childrenByParentId = new Map<string | null, AssetCategory[]>();
  for (const category of categories) {
    const parentId = category.parent_category_id ?? null;
    const children = childrenByParentId.get(parentId) ?? [];
    children.push(category);
    childrenByParentId.set(parentId, children);
  }

  for (const children of childrenByParentId.values()) {
    children.sort((left, right) => left.path.localeCompare(right.path, "zh-CN"));
  }

  return childrenByParentId;
}

function flattenVisibleCategories(
  childrenByParentId: Map<string | null, AssetCategory[]>,
  expandedIds: Set<string>,
) {
  const rows: CategoryTreeRow[] = [];

  function walk(parentId: string | null, depth: number) {
    for (const category of childrenByParentId.get(parentId) ?? []) {
      rows.push({ category, depth });
      if (expandedIds.has(category.category_id)) {
        walk(category.category_id, depth + 1);
      }
    }
  }

  walk(null, 0);
  return rows;
}

function filterParentOptions(
  categories: AssetCategory[],
  editingCategory: AssetCategory | null,
) {
  if (!editingCategory) {
    return categories;
  }

  return categories.filter(
    (category) => !pathIsSelfOrDescendant(category.path, editingCategory.path),
  );
}

function countDescendants(categories: AssetCategory[], root: AssetCategory) {
  return categories.filter(
    (category) =>
      category.category_id !== root.category_id &&
      pathIsSelfOrDescendant(category.path, root.path),
  ).length;
}

function pathIsSelfOrDescendant(candidatePath: string, rootPath: string) {
  return candidatePath === rootPath || candidatePath.startsWith(`${rootPath}.`);
}

function emptyCategoryForm(parentCategoryId = ""): CategoryForm {
  return {
    code: "",
    description: "",
    name: "",
    parameter_assignments: [],
    parent_category_id: parentCategoryId,
  };
}

function emptyParameterAssignmentForm(parameterTypeId = ""): CategoryParameterAssignmentForm {
  return {
    applies_to_descendants: true,
    client_id: makeClientId(),
    is_required: true,
    parameter_type_id: parameterTypeId,
    sort_order: "0",
  };
}

function normalizeParameterAssignments(
  assignments: CategoryParameterAssignmentForm[],
):
  | {
      ok: true;
      parameter_assignments: AssetCategoryParameterAssignmentPayload[];
    }
  | { ok: false; message: string } {
  const parameterAssignments = assignments.map((assignment) => {
    const sortOrder = Number(assignment.sort_order);

    return {
      applies_to_descendants: assignment.applies_to_descendants,
      is_required: assignment.is_required,
      parameter_type_id: assignment.parameter_type_id,
      sort_order: sortOrder,
    };
  });

  if (parameterAssignments.some((assignment) => !assignment.parameter_type_id)) {
    return { ok: false, message: "请选择附带参数" };
  }

  if (parameterAssignments.some((assignment) => !Number.isInteger(assignment.sort_order))) {
    return { ok: false, message: "附带参数排序必须是整数" };
  }

  const parameterIds = new Set<string>();
  for (const assignment of parameterAssignments) {
    if (parameterIds.has(assignment.parameter_type_id)) {
      return { ok: false, message: "同一分类不能重复绑定同一个参数" };
    }
    parameterIds.add(assignment.parameter_type_id);
  }

  return { ok: true, parameter_assignments: parameterAssignments };
}

function makeClientId() {
  return globalThis.crypto?.randomUUID?.() ?? Math.random().toString(36).slice(2);
}
