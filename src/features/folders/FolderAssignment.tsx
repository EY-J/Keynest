import { useEffect, useState } from "react";
import { foldersClient, type Folder } from "./foldersClient";

type Props = {
  kind: "credential" | "note";
  itemId: string;
  onAssigned?(): void;
};

export default function FolderAssignment({ kind, itemId, onAssigned }: Props) {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [folderId, setFolderId] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState(false);

  useEffect(() => {
    let active = true;
    setLoading(true);
    Promise.all([foldersClient.list(), foldersClient.items()])
      .then(([allFolders, items]) => {
        if (!active) return;
        setFolders(allFolders);
        setFolderId(
          items.find((item) => item.kind === kind && item.itemId === itemId)
            ?.folderId ?? "",
        );
        setError("");
      })
      .catch((reason: unknown) => {
        if (active)
          setError(
            reason instanceof Error
              ? reason.message
              : "Could not load folders.",
          );
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [kind, itemId]);

  async function assign(nextId: string) {
    setPending(true);
    setError("");
    try {
      if (kind === "credential")
        await foldersClient.assignCredential(itemId, nextId || null);
      else await foldersClient.assignNote(itemId, nextId || null);
      setFolderId(nextId);
      onAssigned?.();
    } catch (reason) {
      setError(
        reason instanceof Error ? reason.message : "Could not update folder.",
      );
    } finally {
      setPending(false);
    }
  }

  return (
    <div className="folder-assignment">
      <label htmlFor={`folder-${kind}-${itemId}`}>Folder</label>
      <select
        id={`folder-${kind}-${itemId}`}
        value={folderId}
        disabled={loading || pending}
        onChange={(event) => void assign(event.target.value)}
      >
        <option value="">Unfiled</option>
        {folders.map((folder) => (
          <option key={folder.id} value={folder.id}>
            {folder.name}
          </option>
        ))}
      </select>
      {error ? (
        <p className="vault-form-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
