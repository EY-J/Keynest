import { type ReactNode, type RefObject, useEffect, useRef } from "react";

type VaultModalProps = {
  titleId: string;
  onRequestClose: () => void;
  isDismissDisabled?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
  children: ReactNode;
};

const FOCUSABLE_SELECTOR =
  "button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [href]";

export default function VaultModal({
  titleId,
  onRequestClose,
  isDismissDisabled = false,
  initialFocusRef,
  fallbackFocusRef,
  children,
}: VaultModalProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const closeRef = useRef(onRequestClose);
  const dismissDisabledRef = useRef(isDismissDisabled);
  closeRef.current = onRequestClose;
  dismissDisabledRef.current = isDismissDisabled;

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    function focusInside(preferLast = false) {
      const dialog = dialogRef.current;
      if (!dialog) {
        return;
      }
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
      const preferred = initialFocusRef?.current;
      if (!preferLast && preferred && !preferred.hasAttribute("disabled")) {
        preferred.focus();
      } else {
        (preferLast ? focusable[focusable.length - 1] : focusable[0] ?? dialog).focus();
      }
    }
    queueMicrotask(() => focusInside());
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !dismissDisabledRef.current) {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab") {
        return;
      }
      const dialog = dialogRef.current;
      if (!dialog) {
        return;
      }
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) {
        event.preventDefault();
        dialog.focus();
      } else if (!dialog.contains(document.activeElement)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    function handleFocusIn(event: FocusEvent) {
      if (!dialogRef.current?.contains(event.target as Node)) {
        focusInside();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    document.addEventListener("focusin", handleFocusIn);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      document.removeEventListener("focusin", handleFocusIn);
      if (opener?.isConnected) {
        opener.focus();
      } else if (fallbackFocusRef?.current?.isConnected) {
        fallbackFocusRef.current.focus();
      } else {
        document
          .querySelector<HTMLElement>("[data-vault-modal-fallback]")
          ?.focus();
      }
    };
  }, []);

  return (
    <div
      className="vault-dialog-backdrop"
      onMouseDown={(event) => {
        if (
          event.target === event.currentTarget &&
          !dismissDisabledRef.current
        ) {
          closeRef.current();
        }
      }}
    >
      <section
        ref={dialogRef}
        className="vault-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        {children}
      </section>
    </div>
  );
}
