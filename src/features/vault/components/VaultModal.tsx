import type { ReactNode, RefObject } from "react";
import Modal from "../../../components/ui/Modal/Modal";

type VaultModalProps = {
  titleId: string;
  className?: string;
  width?: number;
  onRequestClose: () => void;
  isDismissDisabled?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
  children: ReactNode;
  closing: boolean;
  onExitComplete: () => void;
};

// Vault-specific size only; all motion, focus and dismissal live in Modal.
export default function VaultModal({
  titleId,
  className = "",
  width = 620,
  onRequestClose,
  isDismissDisabled = false,
  initialFocusRef,
  fallbackFocusRef,
  children,
  closing,
  onExitComplete,
}: VaultModalProps) {
  return (
    <Modal
      titleId={titleId}
      className={`vault-dialog ${className}`.trim()}
      width={width}
      closing={closing}
      onClose={onRequestClose}
      onExitComplete={onExitComplete}
      pending={isDismissDisabled}
      closeOnBackdrop
      initialFocusRef={initialFocusRef}
      fallbackFocusRef={fallbackFocusRef}
    >
      {children}
    </Modal>
  );
}
