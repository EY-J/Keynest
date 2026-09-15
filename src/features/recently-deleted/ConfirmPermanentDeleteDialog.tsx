import { useRef } from "react";
import Modal, { useModalClose } from "../../components/ui/Modal/Modal";
import type { DeletedItem } from "./types";

type ConfirmPermanentDeleteDialogProps = {
  item: DeletedItem | null;
  pending: boolean;
  error: string;
  onCancel(): void;
  onConfirm(): Promise<boolean>;
};

export default function ConfirmPermanentDeleteDialog({
  item,
  pending,
  error,
  onCancel,
  onConfirm,
}: ConfirmPermanentDeleteDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const modal = useModalClose(onCancel);
  const close = () => { if (!pending) modal.close(); };
  const confirm = async () => { if (await onConfirm()) modal.close(); };

  return (
    <Modal
      titleId="permanent-delete-title"
      className="recently-deleted-dialog"
      width={460}
      closing={modal.closing}
      onClose={close}
      onExitComplete={modal.finishClose}
      pending={pending}
      closeOnBackdrop
      initialFocusRef={cancelRef}
    >
      <h2 id="permanent-delete-title">
        {item ? "Delete permanently?" : "Empty Recently Deleted?"}
      </h2>
      <p>
        {item
          ? "This item will be permanently removed from KeyNest and cannot be restored."
          : "All items will be permanently removed from KeyNest. This action cannot be undone."}
      </p>
      {item ? <p className="recently-deleted-dialog-name">{item.title}</p> : null}
      {error ? <p className="vault-form-error" role="alert">{error}</p> : null}
      <div className="vault-dialog-actions">
        <button ref={cancelRef} className="secondary-button" type="button" onClick={close} disabled={pending}>
          Cancel
        </button>
        <button className="vault-danger-button" type="button" onClick={() => void confirm()} disabled={pending}>
          Delete permanently
        </button>
      </div>
    </Modal>
  );
}
