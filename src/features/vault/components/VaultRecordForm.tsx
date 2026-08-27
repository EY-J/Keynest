import { type FormEvent, type RefObject, useEffect, useRef, useState } from "react";
import type { VaultRecord, VaultRecordInput } from "../types";
import { generateAdvancedPassword } from "./PasswordGenerator";
import PasswordStrengthMeter from "./PasswordStrengthMeter";

type VaultRecordFormProps = {
  initialRecord?: VaultRecord;
  onSubmit: (input: VaultRecordInput) => Promise<void>;
  onCancel: () => void;
  onPendingChange?: (isPending: boolean) => void;
  initialFocusRef?: RefObject<HTMLInputElement | null>;
};

type RequiredField = "name" | "username" | "password" | "category";

const VAULT_CATEGORIES = [
  "Personal",
  "Email",
  "Social Media",
  "Finance",
  "Work",
  "Shopping",
  "Developer",
  "Network",
  "Government",
  "Gaming",
  "Entertainment",
  "Travel",
  "Utilities",
  "Other",
] as const;

const FIELD_ERROR_IDS: Record<RequiredField, string> = {
  name: "vault-name-error",
  username: "vault-username-error",
  password: "vault-password-error",
  category: "vault-category-error",
};

function normalizeTags(value: string) {
  const tags: string[] = [];
  for (const rawTag of value.split(",")) {
    const tag = rawTag.trim();
    if (
      tag &&
      !tags.some(
        (existing) => existing.toLocaleLowerCase() === tag.toLocaleLowerCase(),
      )
    ) {
      tags.push(tag);
    }
  }
  return tags;
}

// ── Inline SVG icons ────────────────────────────────────────────────────────

function EyeIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

function EyeOffIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path
        d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1
          5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16
          3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"
      />
      <line x1="1" y1="1" x2="23" y2="23" />
    </svg>
  );
}

function SparkleIcon() {
  return (
    <svg
      width="17"
      height="17"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path
        d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1
          1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2
          2 0 0 1-1.275-1.275L12 3Z"
      />
      <path d="M5 3v4" />
      <path d="M19 17v4" />
      <path d="M3 5h4" />
      <path d="M17 19h4" />
    </svg>
  );
}

// ── Form component ───────────────────────────────────────────────────────────

export default function VaultRecordForm({
  initialRecord,
  onSubmit,
  onCancel,
  onPendingChange,
  initialFocusRef,
}: VaultRecordFormProps) {
  const [name, setName] = useState(initialRecord?.name ?? "");
  const [username, setUsername] = useState(initialRecord?.username ?? "");
  const [password, setPassword] = useState(initialRecord?.password ?? "");
  const [website, setWebsite] = useState(initialRecord?.website ?? "");
  const [category, setCategory] = useState(initialRecord?.category ?? "");
  const [tags, setTags] = useState(initialRecord?.tags.join(", ") ?? "");
  const [fieldErrors, setFieldErrors] = useState<Partial<Record<RequiredField, string>>>({});
  const [formError, setFormError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const isMounted = useRef(true);
  const submissionId = useRef(0);
  const categoryOptions =
    initialRecord?.category &&
    !VAULT_CATEGORIES.includes(
      initialRecord.category as (typeof VAULT_CATEGORIES)[number],
    )
      ? [initialRecord.category, ...VAULT_CATEGORIES]
      : VAULT_CATEGORIES;

  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
    };
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const validationErrors: Partial<Record<RequiredField, string>> = {};
    if (!name.trim()) validationErrors.name = "Enter a credential name.";
    if (!username.trim()) validationErrors.username = "Enter a username or email.";
    if (!password) validationErrors.password = "Enter a password.";
    if (!category.trim()) validationErrors.category = "Enter a credential category.";
    if (Object.keys(validationErrors).length) {
      setFieldErrors(validationErrors);
      setFormError("");
      return;
    }

    const currentSubmission = ++submissionId.current;
    setFieldErrors({});
    setFormError("");
    setIsSubmitting(true);
    onPendingChange?.(true);
    try {
      await onSubmit({
        name: name.trim(),
        username: username.trim(),
        password,
        website: website.trim() || null,
        category: category.trim(),
        tags: normalizeTags(tags),
      });
    } catch {
      if (isMounted.current && submissionId.current === currentSubmission) {
        setFormError("KeyNest could not save this credential.");
      }
    } finally {
      if (isMounted.current && submissionId.current === currentSubmission) {
        setIsSubmitting(false);
        onPendingChange?.(false);
      }
    }
  }

  function handleGeneratePassword() {
    setPassword(generateAdvancedPassword());
    setFieldErrors((current) => {
      const { password: _passwordError, ...remainingErrors } = current;
      return remainingErrors;
    });
    setShowPassword(false);
  }

  return (
    <form className="vault-record-form" onSubmit={(event) => void submit(event)}>
      <div className="vault-form-field">
        <label htmlFor="vault-name">Name</label>
        <input
          id="vault-name"
          ref={initialFocusRef}
          placeholder="e.g. Google Account"
          value={name}
          onChange={(event) => setName(event.target.value)}
          disabled={isSubmitting}
          aria-invalid={Boolean(fieldErrors.name)}
          aria-describedby={fieldErrors.name ? FIELD_ERROR_IDS.name : undefined}
        />
        {fieldErrors.name ? (
          <span id={FIELD_ERROR_IDS.name} className="vault-field-error">
            {fieldErrors.name}
          </span>
        ) : null}
      </div>
      <div className="vault-form-field">
        <label htmlFor="vault-username">Username or email</label>
        <input
          id="vault-username"
          placeholder="e.g. user@example.com"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          disabled={isSubmitting}
          autoComplete="username"
          aria-invalid={Boolean(fieldErrors.username)}
          aria-describedby={
            fieldErrors.username ? FIELD_ERROR_IDS.username : undefined
          }
        />
        {fieldErrors.username ? (
          <span id={FIELD_ERROR_IDS.username} className="vault-field-error">
            {fieldErrors.username}
          </span>
        ) : null}
      </div>

      {/* ── Password field ────────────────────────────────────────────────── */}
      <div className="vault-form-field">
        <label htmlFor="vault-password">Password</label>
        <div className="vault-password-input-row">
          <input
            id="vault-password"
            type={showPassword ? "text" : "password"}
            placeholder="Enter your password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            disabled={isSubmitting}
            autoComplete="new-password"
            aria-invalid={Boolean(fieldErrors.password)}
            aria-describedby={fieldErrors.password ? FIELD_ERROR_IDS.password : undefined}
          />
          {/* Show / hide toggle */}
          <button
            type="button"
            className="vault-pw-icon-btn"
            onClick={() => setShowPassword((value) => !value)}
            aria-label={showPassword ? "Hide password" : "Show password"}
            title={showPassword ? "Hide password" : "Show password"}
            disabled={isSubmitting}
          >
            {showPassword ? <EyeOffIcon /> : <EyeIcon />}
          </button>
          {/* Generator trigger */}
          <button
            type="button"
            className="vault-pw-icon-btn"
            onClick={handleGeneratePassword}
            aria-label="Generate password"
            title="Generate a strong password"
            disabled={isSubmitting}
          >
            <SparkleIcon />
          </button>
        </div>

        {/* Inline generator panel */}
        {fieldErrors.password ? (
          <span id={FIELD_ERROR_IDS.password} className="vault-field-error">
            {fieldErrors.password}
          </span>
        ) : null}

        {/* Strength meter — always visible when password is non-empty */}
        {password && <PasswordStrengthMeter password={password} />}
      </div>

      <label>
        <span>Website (optional)</span>
        <input
          type="text"
          inputMode="url"
          placeholder="e.g. https://google.com"
          value={website}
          onChange={(event) => setWebsite(event.target.value)}
          disabled={isSubmitting}
        />
      </label>
      <div className="vault-form-field">
        <label htmlFor="vault-category">Category</label>
        <select
          id="vault-category"
          value={category}
          onChange={(event) => setCategory(event.target.value)}
          disabled={isSubmitting}
          aria-invalid={Boolean(fieldErrors.category)}
          aria-describedby={
            fieldErrors.category ? FIELD_ERROR_IDS.category : undefined
          }
        >
          <option value="">Select a category</option>
          {categoryOptions.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
        {fieldErrors.category ? (
          <span id={FIELD_ERROR_IDS.category} className="vault-field-error">
            {fieldErrors.category}
          </span>
        ) : null}
      </div>
      <label>
        <span>Tags (comma-separated)</span>
        <input
          placeholder="e.g. work, personal"
          value={tags}
          onChange={(event) => setTags(event.target.value)}
          disabled={isSubmitting}
        />
      </label>
      {formError ? (
        <div className="vault-form-error" aria-live="assertive">
          <p>{formError}</p>
        </div>
      ) : null}
      <div className="vault-dialog-actions">
        <button
          className="secondary-button"
          type="button"
          onClick={onCancel}
          disabled={isSubmitting}
        >
          Cancel
        </button>
        <button className="primary-button" disabled={isSubmitting}>
          {isSubmitting ? "Saving…" : "Save Credential"}
        </button>
      </div>
    </form>
  );
}
