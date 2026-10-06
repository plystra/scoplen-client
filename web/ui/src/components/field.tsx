// SPDX-License-Identifier: Apache-2.0
import { useId, type InputHTMLAttributes, type Ref } from "react";

export interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "id"> {
  /** Visible label; placeholders are not labels. */
  label: string;
  /** Guidance shown under the field, such as why it is asked for. */
  hint?: string;
  /** The problem with the current value, shown next to the field. */
  error?: string;
  ref?: Ref<HTMLInputElement>;
}

/**
 * A labelled text input. The hint and the error are announced with the
 * field, and an error marks it invalid without relying on color alone.
 */
export function Field({ label, hint, error, className, ref, ...input }: FieldProps) {
  const id = useId();
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  return (
    <div className={["flex flex-col gap-1.5", className].filter(Boolean).join(" ")}>
      <label htmlFor={id} className="text-sm font-medium text-foreground">
        {label}
      </label>
      <input
        ref={ref}
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={[errorId, hintId].filter(Boolean).join(" ") || undefined}
        className={[
          "h-9 rounded-md border bg-inset px-3 text-sm text-foreground",
          "transition-colors duration-(--duration-micro) focus-visible:outline-2 focus-visible:outline-offset-1",
          error ? "border-attention" : "border-border-strong",
        ].join(" ")}
        {...input}
      />
      {error ? (
        <p id={errorId} className="text-sm text-attention">
          {error}
        </p>
      ) : null}
      {hint ? (
        <p id={hintId} className="text-sm text-muted-foreground">
          {hint}
        </p>
      ) : null}
    </div>
  );
}
