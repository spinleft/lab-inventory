import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { useQueryClient } from "@tanstack/react-query";
import {
  ChevronDown,
  ChevronRight,
  MapPin,
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
import { canManageLocations } from "../auth/permissions";
import {
  adminQueryKeys,
  type Location,
  type LocationPayload,
  optionalText,
  useCreateLocation,
  useDeleteLocation,
  useLocations,
  useUpdateLocation,
} from "./api";

type LocationEditorState =
  | {
      mode: "create";
      parentLocationId: string | null;
    }
  | {
      location: Location;
      mode: "edit";
    };

type LocationForm = {
  code: string;
  description: string;
  name: string;
  parent_location_id: string;
};

type LocationTreeRow = {
  depth: number;
  location: Location;
};

const EMPTY_LOCATIONS: Location[] = [];

export function LocationsPage() {
  const { currentUser } = useAuth();
  const {
    canManageSelectedLaboratoryAssets,
    selectedLaboratoryId,
    selectedLaboratoryName,
  } = useLaboratorySelection();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const canManage = canManageLocations(currentUser);
  const locationsQuery = useLocations({
    enabled: canManageSelectedLaboratoryAssets && Boolean(selectedLaboratoryId),
    laboratoryId: selectedLaboratoryId,
  });
  const createLocation = useCreateLocation();
  const updateLocation = useUpdateLocation();
  const deleteLocation = useDeleteLocation();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
  const [editing, setEditing] = useState<LocationEditorState | null>(null);
  const locations = locationsQuery.data ?? EMPTY_LOCATIONS;
  const childrenByParentId = useMemo(() => buildLocationChildren(locations), [locations]);
  const visibleRows = useMemo(
    () => flattenVisibleLocations(childrenByParentId, expandedIds),
    [childrenByParentId, expandedIds],
  );

  useEffect(() => {
    setExpandedIds(new Set());
  }, [selectedLaboratoryId]);

  function refresh() {
    if (!selectedLaboratoryId) return;
    queryClient.invalidateQueries({
      queryKey: adminQueryKeys.locations(apiBaseUrl, selectedLaboratoryId),
    });
  }

  function toggleLocation(locationId: string) {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(locationId)) {
        next.delete(locationId);
      } else {
        next.add(locationId);
      }
      return next;
    });
  }

  function openCreate(parentLocationId: string | null = null) {
    setEditing({ mode: "create", parentLocationId });
  }

  function handleDelete(location: Location) {
    deleteLocation.mutate(location.location_id, {
      onError: (error) =>
        toast.error({ title: "删除位置失败", description: toErrorMessage(error) }),
      onSuccess: () => {
        refresh();
        toast.success({ title: "位置已删除" });
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
      新建位置
    </Button>
  );

  if (!canManage) {
    return (
      <main className="page">
        <PageHeader
          kicker="管理"
          title="位置"
          description="当前账号没有管理位置的权限。"
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
        title="位置"
        description="按实验室维护存放位置树，位置代码会参与生成层级路径。"
        actions={pageActions}
      />
      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">{selectedLaboratoryName || "未选择实验室"}</h2>
            <p className="panel-description">
              {!selectedLaboratoryId
                ? "请选择实验室后管理位置"
                : canManageSelectedLaboratoryAssets
                  ? `${locations.length} 个位置`
                  : "当前账号不能管理该实验室"}
            </p>
          </div>
        </div>
        <LocationTreeTable
          childrenByParentId={childrenByParentId}
          deleting={deleteLocation.isPending}
          expandedIds={expandedIds}
          loading={locationsQuery.isLoading}
          locations={locations}
          rows={visibleRows}
          onCreateChild={(location) => openCreate(location.location_id)}
          onDelete={handleDelete}
          onEdit={(location) => setEditing({ location, mode: "edit" })}
          onToggle={toggleLocation}
        />
      </section>
      <LocationEditor
        createLocation={createLocation}
        editing={editing}
        laboratoryId={selectedLaboratoryId}
        locations={locations}
        open={editing !== null}
        updateLocation={updateLocation}
        onClose={() => setEditing(null)}
        onSaved={(parentLocationId) => {
          setEditing(null);
          if (parentLocationId) {
            setExpandedIds((current) => new Set(current).add(parentLocationId));
          }
          refresh();
        }}
      />
    </main>
  );
}

function LocationTreeTable({
  childrenByParentId,
  deleting,
  expandedIds,
  loading,
  locations,
  onCreateChild,
  onDelete,
  onEdit,
  onToggle,
  rows,
}: {
  childrenByParentId: Map<string | null, Location[]>;
  deleting: boolean;
  expandedIds: Set<string>;
  loading: boolean;
  locations: Location[];
  onCreateChild: (location: Location) => void;
  onDelete: (location: Location) => void;
  onEdit: (location: Location) => void;
  onToggle: (locationId: string) => void;
  rows: LocationTreeRow[];
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
        description="当前实验室还没有位置。"
        title="暂无位置"
      />
    );
  }

  return (
    <div className="table-wrap">
      <table className="data-table category-tree-table">
        <thead>
          <tr>
            <th>位置</th>
            <th>代码</th>
            <th>描述</th>
            <th>更新时间</th>
            <th style={{ textAlign: "right" }}>操作</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ location, depth }) => {
            const children = childrenByParentId.get(location.location_id) ?? [];
            const hasChildren = children.length > 0;
            const expanded = expandedIds.has(location.location_id);
            const descendantCount = countDescendants(locations, location);

            return (
              <tr key={location.location_id}>
                <td>
                  <div
                    className="category-tree-cell"
                    style={{ paddingLeft: depth * 24 }}
                  >
                    <Button
                      aria-label={expanded ? "收起位置" : "展开位置"}
                      className="category-tree-toggle"
                      disabled={!hasChildren}
                      size="icon"
                      variant="ghost"
                      onClick={() => onToggle(location.location_id)}
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
                    <MapPin size={15} aria-hidden="true" />
                    <div className="category-tree-title">
                      <strong>{location.name}</strong>
                      <span>{location.path}</span>
                    </div>
                  </div>
                </td>
                <td>
                  <Badge>{location.code}</Badge>
                </td>
                <td>{location.description ?? "未填写"}</td>
                <td>{formatDate(location.updated_at)}</td>
                <td style={{ textAlign: "right" }}>
                  <span className="table-actions">
                    <Button
                      aria-label="新增子位置"
                      size="icon"
                      variant="ghost"
                      onClick={() => onCreateChild(location)}
                    >
                      <Plus size={15} />
                    </Button>
                    <Button
                      aria-label="编辑"
                      size="icon"
                      variant="ghost"
                      onClick={() => onEdit(location)}
                    >
                      <Pencil size={15} />
                    </Button>
                    <ConfirmDialog
                      confirmLabel="删除"
                      description={
                        descendantCount > 0
                          ? `删除「${location.name}」会同时删除 ${descendantCount} 个子位置。确认继续？`
                          : `删除「${location.name}」会同时删除所有子位置。当前没有子位置，确认继续？`
                      }
                      disabled={deleting}
                      title="删除位置"
                      trigger={
                        <Button size="icon" variant="ghost" aria-label="删除">
                          <Trash2 size={15} />
                        </Button>
                      }
                      onConfirm={() => onDelete(location)}
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

function LocationEditor({
  createLocation,
  editing,
  laboratoryId,
  locations,
  onClose,
  onSaved,
  open,
  updateLocation,
}: {
  createLocation: ReturnType<typeof useCreateLocation>;
  editing: LocationEditorState | null;
  laboratoryId: string;
  locations: Location[];
  onClose: () => void;
  onSaved: (parentLocationId: string | null) => void;
  open: boolean;
  updateLocation: ReturnType<typeof useUpdateLocation>;
}) {
  const toast = useToast();
  const isNew = editing?.mode === "create";
  const editingLocation = editing?.mode === "edit" ? editing.location : null;
  const [values, setValues] = useState<LocationForm>(emptyLocationForm());
  const parentOptions = useMemo(
    () => filterParentOptions(locations, editingLocation),
    [locations, editingLocation],
  );
  const isSaving = createLocation.isPending || updateLocation.isPending;

  useEffect(() => {
    if (!editing) {
      setValues(emptyLocationForm());
      return;
    }

    if (editing.mode === "create") {
      setValues(emptyLocationForm(editing.parentLocationId ?? ""));
      return;
    }

    setValues({
      code: editing.location.code,
      description: editing.location.description ?? "",
      name: editing.location.name,
      parent_location_id: editing.location.parent_location_id ?? "",
    });
  }, [editing]);

  function updateField(field: keyof LocationForm, value: string) {
    setValues((current) => ({ ...current, [field]: value }));
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!laboratoryId) {
      toast.error({ title: "请先选择实验室" });
      return;
    }

    const payload: LocationPayload = {
      code: values.code.trim(),
      description: optionalText(values.description),
      name: values.name.trim(),
      parent_location_id: values.parent_location_id || null,
    };

    if (!payload.name || !payload.code) {
      toast.error({ title: "请填写位置名和位置代码" });
      return;
    }

    if (isNew) {
      createLocation.mutate(
        { laboratoryId, payload },
        {
          onError: (error) =>
            toast.error({ title: "创建位置失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "位置已创建" });
            onSaved(payload.parent_location_id);
          },
        },
      );
      return;
    }

    if (editingLocation) {
      updateLocation.mutate(
        { locationId: editingLocation.location_id, payload },
        {
          onError: (error) =>
            toast.error({ title: "更新位置失败", description: toErrorMessage(error) }),
          onSuccess: () => {
            toast.success({ title: "位置已更新" });
            onSaved(payload.parent_location_id);
          },
        },
      );
    }
  }

  return (
    <Dialog
      sidePanel
      description="位置代码只能在同一上级位置下保持唯一，保存后会刷新位置树。"
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !isSaving) onClose();
      }}
      open={open}
      title={isNew ? "新建位置" : "编辑位置"}
      footer={
        <>
          <Button disabled={isSaving} onClick={onClose}>
            取消
          </Button>
          <Button disabled={isSaving} form="location-form" type="submit" variant="primary">
            保存
          </Button>
        </>
      }
    >
      <form className="form-grid" id="location-form" onSubmit={handleSubmit}>
        <FormField htmlFor="location-parent" label="上级位置">
          <LocationParentPicker
            id="location-parent"
            locations={parentOptions}
            value={values.parent_location_id}
            onValueChange={(value) => updateField("parent_location_id", value)}
          />
        </FormField>
        <FormField htmlFor="location-name" label="位置名">
          <input
            className="input"
            id="location-name"
            value={values.name}
            onChange={(event) => updateField("name", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="location-code" label="位置代码">
          <input
            className="input"
            id="location-code"
            value={values.code}
            onChange={(event) => updateField("code", event.target.value)}
          />
        </FormField>
        <FormField htmlFor="location-description" label="描述">
          <textarea
            className="textarea"
            id="location-description"
            value={values.description}
            onChange={(event) => updateField("description", event.target.value)}
          />
        </FormField>
      </form>
    </Dialog>
  );
}

function LocationParentPicker({
  id,
  locations,
  onValueChange,
  value,
}: {
  id: string;
  locations: Location[];
  onValueChange: (value: string) => void;
  value: string;
}) {
  const childrenByParentId = useMemo(() => buildLocationChildren(locations), [locations]);
  const selectedLocation = locations.find((location) => location.location_id === value);
  const rootLocations = childrenByParentId.get(null) ?? [];

  function renderLocationOption(location: Location): ReactNode {
    const children = childrenByParentId.get(location.location_id) ?? [];
    const label = (
      <span className="category-parent-option">
        <span>{location.name}</span>
        <span>{location.code}</span>
      </span>
    );

    if (children.length === 0) {
      return (
        <DropdownMenu.Item
          className="dropdown-item"
          key={location.location_id}
          onSelect={() => onValueChange(location.location_id)}
        >
          {label}
        </DropdownMenu.Item>
      );
    }

    return (
      <DropdownMenu.Sub key={location.location_id}>
        <DropdownMenu.SubTrigger className="dropdown-item category-parent-subtrigger">
          {label}
          <ChevronRight size={13} aria-hidden="true" />
        </DropdownMenu.SubTrigger>
        <DropdownMenu.Portal>
          <DropdownMenu.SubContent className="dropdown-content category-parent-menu">
            <DropdownMenu.Item
              className="dropdown-item"
              onSelect={() => onValueChange(location.location_id)}
            >
              选择“{location.name}”
            </DropdownMenu.Item>
            <DropdownMenu.Separator className="dropdown-separator" />
            {children.map(renderLocationOption)}
          </DropdownMenu.SubContent>
        </DropdownMenu.Portal>
      </DropdownMenu.Sub>
    );
  }

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button className="category-parent-trigger" id={id}>
          <span>{selectedLocation ? selectedLocation.name : "无上级位置"}</span>
          <ChevronDown size={14} aria-hidden="true" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="dropdown-content category-parent-menu" align="start">
          <DropdownMenu.Item className="dropdown-item" onSelect={() => onValueChange("")}>
            无上级位置
          </DropdownMenu.Item>
          {rootLocations.length > 0 ? (
            <>
              <DropdownMenu.Separator className="dropdown-separator" />
              {rootLocations.map(renderLocationOption)}
            </>
          ) : null}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function buildLocationChildren(locations: Location[]) {
  const childrenByParentId = new Map<string | null, Location[]>();
  for (const location of locations) {
    const parentId = location.parent_location_id ?? null;
    const children = childrenByParentId.get(parentId) ?? [];
    children.push(location);
    childrenByParentId.set(parentId, children);
  }

  for (const children of childrenByParentId.values()) {
    children.sort((left, right) => left.path.localeCompare(right.path, "zh-CN"));
  }

  return childrenByParentId;
}

function flattenVisibleLocations(
  childrenByParentId: Map<string | null, Location[]>,
  expandedIds: Set<string>,
) {
  const rows: LocationTreeRow[] = [];

  function walk(parentId: string | null, depth: number) {
    for (const location of childrenByParentId.get(parentId) ?? []) {
      rows.push({ depth, location });
      if (expandedIds.has(location.location_id)) {
        walk(location.location_id, depth + 1);
      }
    }
  }

  walk(null, 0);
  return rows;
}

function filterParentOptions(
  locations: Location[],
  editingLocation: Location | null,
) {
  if (!editingLocation) {
    return locations;
  }

  return locations.filter(
    (location) => !pathIsSelfOrDescendant(location.path, editingLocation.path),
  );
}

function countDescendants(locations: Location[], root: Location) {
  return locations.filter(
    (location) =>
      location.location_id !== root.location_id &&
      pathIsSelfOrDescendant(location.path, root.path),
  ).length;
}

function pathIsSelfOrDescendant(candidatePath: string, rootPath: string) {
  return candidatePath === rootPath || candidatePath.startsWith(`${rootPath}.`);
}

function emptyLocationForm(parentLocationId = ""): LocationForm {
  return {
    code: "",
    description: "",
    name: "",
    parent_location_id: parentLocationId,
  };
}
