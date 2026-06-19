import { type PropsWithChildren } from "react";
import { cn } from "../lib/cn";

export function Badge({
  children,
  tone = "default",
}: PropsWithChildren<{ tone?: "default" | "accent" | "success" | "warning" | "danger" }>) {
  return (
    <span
      className={cn(
        "badge",
        tone === "accent" && "badge-accent",
        tone === "success" && "badge-success",
        tone === "warning" && "badge-warning",
        tone === "danger" && "badge-danger",
      )}
    >
      {children}
    </span>
  );
}
