import { type ButtonHTMLAttributes, forwardRef } from "react";
import { cn } from "./cn";

type IconButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
};

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  ({ children, className, label, ...props }, ref) => (
    <button
      ref={ref}
      type="button"
      aria-label={label}
      title={label}
      className={cn("icon-button", className)}
      {...props}
    >
      {children}
    </button>
  ),
);

IconButton.displayName = "IconButton";
