import type { RefObject } from "react";
import Modal, { useModalClose } from "../../components/ui/Modal/Modal";
import FolderAssignment from "./FolderAssignment";

type Props = {
  kind: "credential" | "note";
  itemId: string;
  onClose(): void;
  fallbackFocusRef?: RefObject<HTMLElement | null>;
};

export default function FolderAssignmentDialog({
  kind,
  itemId,
  onClose,
  fallbackFocusRef,
}: Props) {
  const modal = useModalClose(onClose);

  return (
    <Modal
      titleId="folder-assignment-title"
      className="folder-assignment-dialog"
      width={420}
      closing={modal.closing}
      onClose={() => modal.close()}
      onExitComplete={modal.finishClose}
      closeOnBackdrop
      fallbackFocusRef={fallbackFocusRef}
    >
      <h2 id="folder-assignment-title">Add to folder</h2>
      <p>Choose a folder for this {kind}. Select Unfiled to remove it from a folder.</p>
      <FolderAssignment kind={kind} itemId={itemId} onAssigned={() => modal.close()} />
      <div className="vault-dialog-actions">
        <button className="secondary-button" type="button" onClick={() => modal.close()}>
          Cancel
        </button>
      </div>
    </Modal>
  );
}
