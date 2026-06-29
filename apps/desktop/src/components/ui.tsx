import {Alert, Description, FieldError, Input, Label, TextField as HeroTextField} from "@heroui/react";
import type {InputHTMLAttributes, ReactNode} from "react";

type FieldProps = {
  className?: string;
  /** Visible label. Omit for a label-less field and pass `ariaLabel` instead. */
  label?: string;
  /** Accessible name when there is no visible `label`. */
  ariaLabel?: string;
  value?: string;
  onValueChange?: (value: string) => void;
  placeholder?: string;
  type?: InputHTMLAttributes<HTMLInputElement>["type"];
  inputMode?: InputHTMLAttributes<HTMLInputElement>["inputMode"];
  onKeyDown?: InputHTMLAttributes<HTMLInputElement>["onKeyDown"];
  /** Helper text shown under the field; hidden automatically while `error` is set. */
  description?: ReactNode;
  /** When set, the field is marked invalid and the message is shown inline. */
  error?: ReactNode;
  required?: boolean;
  isDisabled?: boolean;
  autoFocus?: boolean;
};

export function TextField({
  className = "",
  label,
  ariaLabel,
  value,
  onValueChange,
  placeholder,
  type,
  inputMode,
  onKeyDown,
  description,
  error,
  required,
  isDisabled,
  autoFocus,
}: FieldProps) {
  const invalid = Boolean(error);
  return (
    <HeroTextField
      aria-label={label ? undefined : ariaLabel}
      className={`min-w-0 ${className}`}
      isDisabled={isDisabled}
      isInvalid={invalid}
      isRequired={required}
      value={value}
      onChange={onValueChange}
    >
      {label ? <Label>{label}</Label> : null}
      <Input
        autoFocus={autoFocus}
        inputMode={inputMode}
        placeholder={placeholder}
        type={type}
        onKeyDown={onKeyDown}
      />
      {invalid ? (
        <FieldError>{error}</FieldError>
      ) : description ? (
        <Description>{description}</Description>
      ) : null}
    </HeroTextField>
  );
}

export function HelpText({children}: {children: ReactNode}) {
  return <p className="text-muted text-xs leading-5">{children}</p>;
}

/** Consistent, accessible inline error surface. Renders nothing when empty. */
export function ErrorAlert({message, className = ""}: {message?: ReactNode; className?: string}) {
  if (!message) return null;
  return (
    <Alert className={className} status="danger">
      <Alert.Indicator />
      <Alert.Content>
        <Alert.Description>{message}</Alert.Description>
      </Alert.Content>
    </Alert>
  );
}
