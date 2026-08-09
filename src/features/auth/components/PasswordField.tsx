import { type Ref, useId, useState } from "react";

type PasswordFieldProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoComplete: string;
  autoFocus?: boolean;
  disabled?: boolean;
  inputRef?: Ref<HTMLInputElement>;
};

export default function PasswordField({
  label,
  value,
  onChange,
  autoComplete,
  autoFocus = false,
  disabled = false,
  inputRef,
}: PasswordFieldProps) {
  const inputId = useId();
  const [isVisible, setIsVisible] = useState(false);
  const lowerCaseLabel = label.toLocaleLowerCase();

  return (
    <div className="auth-field">
      <label htmlFor={inputId}>{label}</label>
      <div className="password-input-wrap">
        <input
          ref={inputRef}
          id={inputId}
          type={isVisible ? "text" : "password"}
          value={value}
          autoComplete={autoComplete}
          autoFocus={autoFocus}
          disabled={disabled}
          onChange={(event) => onChange(event.target.value)}
        />
        <button
          className="password-reveal-button"
          type="button"
          disabled={disabled}
          aria-label={`${isVisible ? "Hide" : "Show"} ${lowerCaseLabel}`}
          onClick={() => setIsVisible((visible) => !visible)}
        >
          {isVisible ? "Hide" : "Show"}
        </button>
      </div>
    </div>
  );
}
