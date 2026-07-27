import * as React from "react";
import { cn } from "@/lib/utils";

interface FieldProps extends React.HTMLAttributes<HTMLDivElement> {
  label?: string;
  description?: string;
  error?: string | null;
}

export function Field({
  label,
  description,
  error,
  className,
  children,
  ...props
}: FieldProps) {
  return (
    <div className={cn("space-y-1 w-full", className)} {...props}>
      {label && (
        <label className="text-xs font-medium text-foreground tracking-tight">
          {label}
        </label>
      )}
      <div>{children}</div>
      {error && <p className="text-[10px] font-medium text-destructive">{error}</p>}
      {description && !error && (
        <p className="text-[10px] text-muted-foreground leading-normal">
          {description}
        </p>
      )}
    </div>
  );
}
