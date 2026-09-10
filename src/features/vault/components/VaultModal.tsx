import { type ReactNode, type RefObject } from "react";
import Modal from "../../../shared/components/Modal/Modal";

// Vault-specific size only; all motion, focus and dismissal live in Modal.
export default function VaultModal({ titleId, className = "", width = 620, onRequestClose, isDismissDisabled = false,
  initialFocusRef, fallbackFocusRef, children, closing, onExitComplete }: {
  titleId: string; className?: string; width?: number; onRequestClose: () => void; isDismissDisabled?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>; fallbackFocusRef?: RefObject<HTMLElement | null>;
  children: ReactNode; closing: boolean; onExitComplete: () => void;
}) {
  return <Modal titleId={titleId} className={`vault-dialog ${className}`.trim()} width={width}
    closing={closing} onClose={onRequestClose} onExitComplete={onExitComplete}
    pending={isDismissDisabled} closeOnBackdrop initialFocusRef={initialFocusRef} fallbackFocusRef={fallbackFocusRef}>
    {children}
  </Modal>;
}
