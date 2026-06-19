import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "../lib/cn";

type SelectOption = {
  label: string;
  value: string;
};

type SelectProps = {
  disabled?: boolean;
  id?: string;
  label?: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  value?: string;
};

export function Select({
  disabled,
  id,
  label,
  onValueChange,
  options,
  placeholder = "请选择",
  value,
}: SelectProps) {
  return (
    <SelectPrimitive.Root
      disabled={disabled}
      value={value}
      onValueChange={onValueChange}
    >
      <SelectPrimitive.Trigger id={id} aria-label={label} className="select-trigger">
        <SelectPrimitive.Value placeholder={placeholder} />
        <SelectPrimitive.Icon>
          <ChevronDown size={14} />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Content className="select-content" position="popper">
          <SelectPrimitive.Viewport>
            {options.map((option) => (
              <SelectPrimitive.Item
                className="select-item"
                key={option.value}
                value={option.value}
              >
                <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
                <SelectPrimitive.ItemIndicator className={cn("select-item-indicator")}>
                  <Check size={14} />
                </SelectPrimitive.ItemIndicator>
              </SelectPrimitive.Item>
            ))}
          </SelectPrimitive.Viewport>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
