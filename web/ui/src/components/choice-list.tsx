// SPDX-License-Identifier: Apache-2.0
import { useId } from "react";

export interface Choice<T extends string> {
  value: T;
  label: string;
  /** One line explaining the choice. */
  description?: string;
}

export interface ChoiceListProps<T extends string> {
  legend: string;
  choices: Choice<T>[];
  value: T;
  onChange: (value: T) => void;
}

/**
 * A choice among a few options that need a line of explanation each, shown
 * as a vertical list of radio buttons. Lighter than `Segmented`, so it reads
 * as a sub-choice when it sits under one.
 */
export function ChoiceList<T extends string>({ legend, choices, value, onChange }: ChoiceListProps<T>) {
  const name = useId();
  return (
    <fieldset className="m-0 flex flex-col gap-2 border-0 p-0">
      <legend className="mb-1 p-0 text-sm font-medium text-foreground">{legend}</legend>
      {choices.map((choice) => (
        <label key={choice.value} className="flex cursor-pointer items-start gap-2.5 text-sm">
          <input
            type="radio"
            name={name}
            value={choice.value}
            checked={choice.value === value}
            onChange={() => onChange(choice.value)}
            className="mt-0.5 size-4 shrink-0 accent-[var(--primary)]"
          />
          <span className="flex flex-col">
            <span className="text-foreground">{choice.label}</span>
            {choice.description ? <span className="text-muted-foreground">{choice.description}</span> : null}
          </span>
        </label>
      ))}
    </fieldset>
  );
}
