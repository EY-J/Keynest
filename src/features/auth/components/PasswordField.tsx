import { type Ref, useId } from "react";

type PasswordFieldProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoComplete: string;
  autoFocus?: boolean;
  disabled?: boolean;
  inputRef?: Ref<HTMLInputElement>;
  visuallyHideLabel?: boolean;
};

export default function PasswordField({
  label,
  value,
  onChange,
  autoComplete,
  autoFocus = false,
  disabled = false,
  inputRef,
  visuallyHideLabel = false,
}: PasswordFieldProps) {
  const inputId = useId();

  return (
    <div className="auth-field">
      <label className={visuallyHideLabel ? "sr-only" : undefined} htmlFor={inputId}>{label}</label>
      <input
        ref={inputRef}
        id={inputId}
        type="password"
        value={value}
        autoComplete={autoComplete}
        autoFocus={autoFocus}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    </div>
  );
}
