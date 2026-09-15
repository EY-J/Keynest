import { type FormEvent, useEffect, useRef, useState } from "react";
import { KeyRound } from "lucide-react";
import Modal, { ModalCloseButton, useModalClose } from "../../../components/ui/Modal/Modal";
import { authClient } from "../../auth/authClient";
import PasswordField from "../../auth/components/PasswordField";
import { AuthClientError, type PinStatus } from "../../auth/types";
import SettingsRow from "./SettingsRow";
import "./DevicePinSettings.css";

type Mode = "setup" | "manage" | "change" | "remove";

export default function DevicePinSettings() {
  const [status, setStatus] = useState<PinStatus>({ configured: false, unlockAvailable: false });
  const [loading, setLoading] = useState(true);
  const [statusError, setStatusError] = useState(false);
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<Mode>("setup");
  const [masterPassword, setMasterPassword] = useState("");
  const [pin, setPin] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const masterRef = useRef<HTMLInputElement>(null);
  const submittingRef = useRef(false);
  const modal = useModalClose(() => {
    clearSecrets();
    setOpen(false);
  });

  useEffect(() => {
    let current = true;
    void authClient.getPinStatus().then(
      (next) => { if (current) setStatus(next); },
      () => { if (current) setStatusError(true); },
    ).finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, []);

  function clearSecrets() {
    setMasterPassword("");
    setPin("");
    setConfirmation("");
    setError("");
  }

  function openModal() {
    clearSecrets();
    setSuccess("");
    setMode(status.configured ? "manage" : "setup");
    setOpen(true);
  }

  function selectMode(next: Mode) {
    clearSecrets();
    setMode(next);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submittingRef.current || modal.closing) return;
    setError("");
    if (!masterPassword) {
      setError("Enter your current Master Password.");
      return;
    }
    if (mode !== "remove" && (pin.length !== 6 || !/^\d{6}$/.test(pin))) {
      setError("Enter a 6-digit PIN.");
      return;
    }
    if (mode !== "remove" && pin !== confirmation) {
      setError("The PINs do not match.");
      return;
    }

    submittingRef.current = true;
    setSubmitting(true);
    try {
      const next = mode === "setup"
        ? await authClient.setupPin(masterPassword, pin, confirmation)
        : mode === "change"
          ? await authClient.changePin(masterPassword, pin, confirmation)
          : await authClient.removePin(masterPassword);
      setStatus(next);
      setSuccess(mode === "remove" ? "Device PIN removed." : mode === "change" ? "Device PIN changed." : "Device PIN enabled.");
      modal.close();
    } catch (requestError) {
      setError(requestError instanceof AuthClientError
        ? requestError.message
        : "KeyNest could not update the device PIN.");
    } finally {
      setMasterPassword("");
      setPin("");
      setConfirmation("");
      submittingRef.current = false;
      setSubmitting(false);
    }
  }

  function close() {
    if (!submittingRef.current) modal.close();
  }

  return (
    <>
      <SettingsRow
        icon={KeyRound}
        title="Device PIN"
        description={loading ? "Checking status…" : statusError ? "Status unavailable" : status.configured ? "Enabled on this Windows account" : "Not set up"}
      >
        <button
          ref={triggerRef}
          className="secondary-button compact-button"
          type="button"
          disabled={loading || statusError}
          aria-haspopup="dialog"
          aria-expanded={open}
          onClick={openModal}
        >
          {status.configured ? "Manage" : "Set up"}
        </button>
      </SettingsRow>
      {success && !open ? <p className="security-success" role="status">{success}</p> : null}

      {open ? (
        <Modal
          id="device-pin-dialog"
          className="master-password-modal device-pin-modal"
          titleId="device-pin-title"
          closing={modal.closing}
          onClose={close}
          onExitComplete={modal.finishClose}
          pending={submitting}
          closeOnBackdrop
          initialFocusRef={mode === "manage" ? undefined : masterRef}
          fallbackFocusRef={triggerRef}
        >
          <header className="master-password-modal__header">
            <h2 id="device-pin-title">Device PIN</h2>
            <ModalCloseButton label="Close Device PIN settings" disabled={submitting || modal.closing} onClick={close} />
          </header>

          {mode === "manage" ? (
            <div className="device-pin-manage">
              <div className="device-pin-status"><span aria-hidden="true" />Enabled on this device</div>
              <p className="device-pin-description">Use a PIN for faster access on this Windows device.</p>
              <div className="device-pin-actions">
                <button className="secondary-button" type="button" onClick={() => selectMode("change")}>Change PIN</button>
                <button className="danger-button" type="button" onClick={() => selectMode("remove")}>Remove PIN</button>
              </div>
            </div>
          ) : (
            <form className="master-password-modal__content" onSubmit={(event) => void submit(event)}>
              <p className="device-pin-warning">
                {mode === "remove"
                  ? "Removing the PIN leaves your Master Password and Recovery Key unchanged."
                  : "A PIN is a lower-assurance convenience unlock. It works only on this Windows account; your Master Password remains the primary credential."}
              </p>
              <PasswordField
                label="Current Master Password"
                value={masterPassword}
                onChange={setMasterPassword}
                autoComplete="current-password"
                disabled={submitting || modal.closing}
                inputRef={masterRef}
              />
              {mode !== "remove" ? (
                <>
                  <PinField label={mode === "change" ? "New 6-digit PIN" : "6-digit PIN"} value={pin} onChange={setPin} disabled={submitting || modal.closing} />
                  <PinField label="Confirm PIN" value={confirmation} onChange={setConfirmation} disabled={submitting || modal.closing} />
                  {confirmation && confirmation !== pin ? <p className="master-password-modal__mismatch" role="status">PINs do not match.</p> : null}
                </>
              ) : null}
              {error ? <p className="master-password-modal__error" role="alert">{error}</p> : null}
              <div className="master-password-modal__footer">
                <button className="secondary-button compact-button" type="button" disabled={submitting || modal.closing} onClick={() => status.configured ? selectMode("manage") : close()}>Cancel</button>
                <button
                  className={`${mode === "remove" ? "danger-button" : "primary-button"} compact-button`}
                  disabled={submitting || !masterPassword || (mode !== "remove" && (pin.length !== 6 || confirmation !== pin))}
                >
                  {submitting ? "Saving…" : mode === "remove" ? "Remove PIN" : mode === "change" ? "Change PIN" : "Enable PIN"}
                </button>
              </div>
            </form>
          )}
        </Modal>
      ) : null}
    </>
  );
}

function PinField({ label, value, onChange, disabled }: { label: string; value: string; onChange: (value: string) => void; disabled: boolean }) {
  const id = label.toLowerCase().replace(/\W+/g, "-");
  return (
    <div className="auth-field auth-pin-field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="password"
        inputMode="numeric"
        pattern="[0-9]{6}"
        maxLength={6}
        autoComplete="off"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value.replace(/\D/g, "").slice(0, 6))}
      />
    </div>
  );
}
