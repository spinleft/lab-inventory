import { Slot } from "@radix-ui/react-slot";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../lib/cn";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  asChild?: boolean;
  size?: "md" | "icon";
  variant?: "default" | "primary" | "danger" | "ghost";
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { asChild = false, className, size = "md", type = "button", variant = "default", ...props },
  ref,
) {
  const Component = asChild ? Slot : "button";
  return (
    <Component
      ref={ref}
      className={cn(
        "button",
        variant === "primary" && "button-primary",
        variant === "danger" && "button-danger",
        variant === "ghost" && "button-ghost",
        size === "icon" && "icon-button",
        className,
      )}
      type={asChild ? undefined : type}
      {...props}
    />
  );
});
