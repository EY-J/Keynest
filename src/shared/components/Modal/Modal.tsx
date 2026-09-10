import { type CSSProperties, type ReactNode, type RefObject, useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import "./modal.css";

// Owners retain their own form state until the shared exit completes. Security
// lock/navigation unmounts still tear down immediately; never delay authorization.
export function useModalClose(onClosed: () => void) {
  const [closing, setClosing] = useState(false);
  const action = useRef<(() => void) | null>(null);
  const closed = useRef(onClosed);
  closed.current = onClosed;
  useEffect(() => () => { action.current = null; }, []);
  return {
    closing,
    close(afterExit?: () => void) {
      if (action.current) return;
      action.current = afterExit ?? (() => closed.current());
      setClosing(true);
    },
    finishClose() {
      const done = action.current;
      action.current = null;
      setClosing(false);
      done?.();
    },
  };
}

let scrollLocks = 0;
let previousOverflow = "";
function lockScroll() {
  if (scrollLocks++ === 0) {
    previousOverflow = document.documentElement.style.overflow;
    document.documentElement.style.overflow = "hidden";
    document.documentElement.classList.add("keynest-modal-active");
  }
  return () => {
    if (--scrollLocks === 0) {
      document.documentElement.style.overflow = previousOverflow;
      document.documentElement.classList.remove("keynest-modal-active");
    }
  };
}

type ModalProps = {
  children: ReactNode;
  titleId: string;
  id?: string;
  className?: string;
  width?: number;
  closing: boolean;
  onClose: () => void;
  onExitComplete: () => void;
  pending?: boolean;
  closeOnBackdrop?: boolean;
  closeOnEscape?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
};
const FOCUSABLE = 'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])';

export function ModalCloseButton({ onClick, disabled, label = "Close", buttonRef, className = "" }: {
  onClick: () => void; disabled?: boolean; label?: string; buttonRef?: RefObject<HTMLButtonElement | null>; className?: string;
}) {
  return <button ref={buttonRef} className={`keynest-modal__close ${className}`.trim()} type="button" aria-label={label} disabled={disabled} onClick={onClick}>
    <X size={20} strokeWidth={2.5} aria-hidden="true" />
  </button>;
}

export default function Modal({ children, titleId, id, className = "", width = 520,
  closing, onClose, onExitComplete, pending = false, closeOnBackdrop = false,
  closeOnEscape = true, initialFocusRef, fallbackFocusRef }: ModalProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const pressedBackdrop = useRef(false);
  const exit = useRef(onExitComplete);
  exit.current = onExitComplete;
  useEffect(() => {
    const dialog = dialogRef.current!;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const unlockScroll = lockScroll();
    dialog.showModal(); // Top layer: background is inert and ancestor overflow cannot clip it.
    const timer = window.setTimeout(() => {
      const preferred = initialFocusRef?.current;
      const firstField = dialog.querySelector<HTMLElement>('input:not(:disabled), textarea:not(:disabled), select:not(:disabled)');
      (preferred ?? firstField ?? dialog.querySelector<HTMLElement>(FOCUSABLE) ?? dialog).focus({ preventScroll: true });
    }, 0);
    return () => {
      window.clearTimeout(timer);
      dialog.close();
      unlockScroll();
      const target = opener?.isConnected ? opener : fallbackFocusRef?.current;
      if (target?.isConnected) target.focus({ preventScroll: true });
    };
  }, []);
  useEffect(() => {
    if (!closing) return;
    // CSS is the source of timing, including reduced motion. Timer covers a
    // missing/cancelled animationend event without an animation loop.
    const durations = getComputedStyle(dialogRef.current!).animationDuration.split(",");
    const duration = Math.max(0, ...durations.map(value => parseFloat(value) * (value.trim().endsWith("ms") ? 1 : 1000)).filter(Number.isFinite));
    const timer = window.setTimeout(() => exit.current(), duration + 50);
    return () => window.clearTimeout(timer);
  }, [closing]);
  const dismiss = () => { if (!pending && !closing) onClose(); };
  function outside(event: { currentTarget: HTMLDialogElement; target: EventTarget; clientX: number; clientY: number }) {
    const rect = event.currentTarget.getBoundingClientRect();
    return event.target === event.currentTarget && (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom);
  }
  return <dialog ref={dialogRef} id={id} className={`keynest-modal ${className}`}
    style={{ "--modal-width": `${width}px` } as CSSProperties}
    data-state={closing ? "closing" : "open"} aria-labelledby={titleId} aria-modal="true" aria-busy={pending} tabIndex={-1}
    onCancel={event => { event.preventDefault(); if (closeOnEscape) dismiss(); }}
    onPointerDown={event => { pressedBackdrop.current = outside(event); }}
    onClick={event => {
      if (closeOnBackdrop && pressedBackdrop.current && outside(event)) dismiss();
      pressedBackdrop.current = false;
    }}
    onKeyDown={event => {
      if (event.key !== "Tab") return;
      const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE))
        .filter(element => element.getClientRects().length > 0 && !element.closest('[inert]'));
      const first = controls[0], last = controls[controls.length - 1];
      if (!first) { event.preventDefault(); event.currentTarget.focus({ preventScroll: true }); }
      else if (!controls.includes(document.activeElement as HTMLElement) || (event.shiftKey ? document.activeElement === first : document.activeElement === last)) {
        event.preventDefault(); (event.shiftKey ? last : first).focus({ preventScroll: true });
      }
    }}
    onAnimationEnd={event => {
      if (closing && event.target === event.currentTarget && !event.nativeEvent.pseudoElement
        && (event.animationName === "keynest-modal-exit" || event.animationName === "keynest-modal-fade-out")) exit.current();
    }}>
    <div className="keynest-modal__body" inert={closing}>{children}</div>
  </dialog>;
}
