import { useRef } from "react";
import { FileText } from "lucide-react";
import Modal, { useModalClose } from "../../../components/ui/Modal/Modal";
import type { Note } from "../types";

type DeleteNoteDialogProps = {
  note: Note;
  pending: boolean;
  error: string;
  fallbackFocusRef: React.RefObject<HTMLButtonElement | null>;
  onCancel(): void;
  onConfirm(): Promise<boolean>;
};

export default function DeleteNoteDialog({ note, pending, error, fallbackFocusRef, onCancel, onConfirm }: DeleteNoteDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const modal = useModalClose(onCancel);
  const close = () => { if (!pending) modal.close(); };
  const confirm = async () => {
    if (await onConfirm()) modal.close();
  };

  return (
    <Modal
      titleId="delete-note-title"
      className="notes-delete-dialog"
      width={440}
      closing={modal.closing}
      onClose={close}
      onExitComplete={modal.finishClose}
      pending={pending}
      closeOnBackdrop
      initialFocusRef={cancelRef}
      fallbackFocusRef={fallbackFocusRef}
    >
      <h2 id="delete-note-title">Delete note?</h2>
      <p className="notes-delete-message">This note will move to Recently Deleted for 30 days.</p>
      <div className="notes-delete-preview">
        <span className="notes-delete-preview-icon" aria-hidden="true">
          <FileText size={20} strokeWidth={1.8} />
        </span>
        <div className="notes-delete-preview-copy">
          <p className="notes-delete-preview-title" title={note.title}>{note.title}</p>
          <p className="notes-delete-preview-type">Note</p>
        </div>
      </div>
      {error ? <p className="vault-form-error" role="alert">{error}</p> : null}
      <div className="vault-dialog-actions">
        <button ref={cancelRef} className="secondary-button" type="button" onClick={close} disabled={pending}>Cancel</button>
        <button className="vault-danger-button" type="button" onClick={() => void confirm()} disabled={pending}>Delete</button>
      </div>
    </Modal>
  );
}
