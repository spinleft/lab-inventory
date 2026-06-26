import { type ReactNode } from "react";
import { type AssetCategory, type Location, type Unit } from "../admin/api";
import {
  type AssetParameterValue,
  type AssetTrackingMode,
} from "../assets/api";
import { type InventoryStatus } from "./api";

export function trackingModeLabel(mode: AssetTrackingMode) {
  return mode === "serialized" ? "序列号管理" : "数量管理";
}

export function inventoryStatusLabel(status: InventoryStatus | string) {
  const labels: Record<string, string> = {
    available: "可用",
    consumed: "已消耗",
    lost: "丢失",
    reserved: "预留",
    retired: "退役",
  };
  return labels[status] ?? status;
}

export function inventoryStatusTone(status: InventoryStatus | string) {
  if (status === "available") return "success" as const;
  if (status === "reserved") return "warning" as const;
  if (status === "retired" || status === "lost" || status === "consumed") {
    return "danger" as const;
  }
  return "default" as const;
}

export function parameterTypeLabel(type: AssetParameterValue["data_type"]) {
  const labels: Record<AssetParameterValue["data_type"], string> = {
    boolean: "布尔",
    date: "日期",
    enum: "枚举",
    number: "数字",
    range: "范围",
    text: "文本",
  };
  return labels[type];
}

export function formatNumber(value: number) {
  return new Intl.NumberFormat("zh-CN", {
    maximumFractionDigits: 4,
  }).format(value);
}

export function formatParameterValue(value: AssetParameterValue, unitsById: Map<string, Unit>) {
  const runtimeValue = value.value;
  if (value.data_type === "text") {
    return runtimeValue.text ?? "";
  }
  if (value.data_type === "number") {
    const unit = runtimeValue.unit_id ? unitsById.get(runtimeValue.unit_id) : null;
    return `${formatNumber(runtimeValue.number ?? 0)}${unit ? ` ${unit.symbol}` : ""}`;
  }
  if (value.data_type === "range") {
    const unit = runtimeValue.unit_id ? unitsById.get(runtimeValue.unit_id) : null;
    return `${formatNumber(runtimeValue.range_start ?? 0)} - ${formatNumber(
      runtimeValue.range_end ?? 0,
    )}${unit ? ` ${unit.symbol}` : ""}`;
  }
  if (value.data_type === "boolean") {
    return runtimeValue.boolean ? "是" : "否";
  }
  if (value.data_type === "date") {
    return runtimeValue.date ?? "";
  }
  if (value.data_type === "enum") {
    return runtimeValue.option_label ?? runtimeValue.option_code ?? runtimeValue.option_id ?? "";
  }
  return "";
}

export function categoryLabel(categoryId: string | null, categoryById: Map<string, AssetCategory>) {
  if (!categoryId) {
    return "未分类";
  }
  return categoryNamePath(categoryId, categoryById) ?? "未知分类";
}

export function locationLabel(locationId: string | null, locationById: Map<string, Location>) {
  if (!locationId) {
    return "未设置";
  }
  return locationNamePath(locationId, locationById) ?? "未知位置";
}

function categoryNamePath(categoryId: string, categoryById: Map<string, AssetCategory>) {
  const names: string[] = [];
  let current = categoryById.get(categoryId);
  const seen = new Set<string>();
  while (current && !seen.has(current.category_id)) {
    seen.add(current.category_id);
    names.unshift(current.name);
    current = current.parent_category_id
      ? categoryById.get(current.parent_category_id)
      : undefined;
  }
  return names.length > 0 ? names.join(" / ") : null;
}

function locationNamePath(locationId: string, locationById: Map<string, Location>) {
  const names: string[] = [];
  let current = locationById.get(locationId);
  const seen = new Set<string>();
  while (current && !seen.has(current.location_id)) {
    seen.add(current.location_id);
    names.unshift(current.name);
    current = current.parent_location_id
      ? locationById.get(current.parent_location_id)
      : undefined;
  }
  return names.length > 0 ? names.join(" / ") : null;
}

export function unitLabel(unitId: string, unitsById: Map<string, Unit>) {
  const unit = unitsById.get(unitId);
  return unit ? `${unit.name} (${unit.symbol})` : "未知单位";
}

export function valueOrMuted(value: ReactNode, fallback = "未填写") {
  if (value === null || value === undefined || value === "") {
    return fallback;
  }
  return value;
}

export function parameterColumnKey(parameterId: string) {
  return `param:${parameterId}`;
}
