export const UNIT_DIMENSION_OPTIONS = [
  { label: "数量", value: "count" },
  { label: "长度", value: "length" },
  { label: "面积", value: "area" },
  { label: "体积", value: "volume" },
  { label: "质量", value: "mass" },
  { label: "时间", value: "time" },
  { label: "温度", value: "temperature" },
  { label: "电流", value: "current" },
  { label: "光强", value: "luminous_intensity" },
  { label: "频率", value: "frequency" },
  { label: "功率", value: "power" },
  { label: "压力", value: "pressure" },
  { label: "能量", value: "energy" },
  { label: "力", value: "force" },
  { label: "扭矩", value: "torque" },
  { label: "密度", value: "density" },
];

export const DEFAULT_UNIT_DIMENSION = "length";

export function unitDimensionLabel(dimension: string) {
  return (
    UNIT_DIMENSION_OPTIONS.find((option) => option.value === dimension)?.label ?? dimension
  );
}
