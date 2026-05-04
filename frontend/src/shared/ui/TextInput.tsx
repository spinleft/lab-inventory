import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "./cn";

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input ref={ref} className={cn("input", className)} {...props} />
  ),
);

TextInput.displayName = "TextInput";
