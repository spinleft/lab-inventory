import { type ReactNode } from "react";

type FormFieldProps = {
  children: ReactNode;
  error?: string;
  hint?: string;
  htmlFor?: string;
  label: string;
};

export function FormField({ children, error, hint, htmlFor, label }: FormFieldProps) {
  return (
    <label className="field" htmlFor={htmlFor}>
      <span className="field-row">
        <span className="field-label">{label}</span>
      </span>
      {children}
      {error ? <p className="field-error">{error}</p> : null}
      {!error && hint ? <p className="field-hint">{hint}</p> : null}
    </label>
  );
}
