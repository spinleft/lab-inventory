import { type SelectHTMLAttributes } from "react";
import { cn } from "./cn";

export function NativeSelect({
  className,
  children,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className={cn("input", "select-native", className)} {...props}>
      {children}
    </select>
  );
}
