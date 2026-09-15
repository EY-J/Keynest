import { useEffect, useMemo, useState } from "react";
import { MoreHorizontal, Search, Trash2 } from "lucide-react";
import ConfirmPermanentDeleteDialog from "./ConfirmPermanentDeleteDialog";
import { recentlyDeletedClient } from "./recentlyDeletedClient";
import type { DeletedItem } from "./types";
import { useToast } from "../../components/ui/Toast/ToastProvider";

const dayFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const exactFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short", day: "numeric", year: "numeric", hour: "numeric", minute: "2-digit",
});

function deletedDate(timestamp: number) {
  const date = new Date(timestamp);
  if (!Number.isFinite(timestamp) || Number.isNaN(date.getTime())) return "Deleted date unavailable";
  const today = new Date();
  const startToday = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const startDate = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const days = Math.round((startToday - startDate) / 86_400_000);
  if (days === 0) return "Deleted today";
  if (days === 1) return "Deleted yesterday";
  return `Deleted ${dayFormatter.format(date)}`;
}

function deletedDateTime(timestamp: number) {
  const date = new Date(timestamp);
  return Number.isFinite(timestamp) && !Number.isNaN(date.getTime())
    ? { dateTime: date.toISOString(), title: exactFormatter.format(date) }
    : { dateTime: undefined, title: undefined };
}

function itemKey(item: DeletedItem) {
  return `${item.itemType}:${item.id}`;
}

export default function RecentlyDeletedPage() {
  const [items, setItems] = useState<DeletedItem[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [openMenuKey, setOpenMenuKey] = useState("");
  const [confirming, setConfirming] = useState<DeletedItem | "all" | null>(null);
  const [actionPending, setActionPending] = useState(false);
  const [actionError, setActionError] = useState("");
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const { showToast } = useToast();

  async function loadItems() {
    setLoading(true);
    setError("");
    try {
      setItems(await recentlyDeletedClient.list());
      setSelectedKeys(new Set());
    } catch {
      setError("KeyNest could not load Recently Deleted.");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void loadItems(); }, []);
  useEffect(() => {
    if (!openMenuKey) return;
    const close = (event: PointerEvent) => {
      if (!(event.target as Element).closest(".recently-deleted-row-actions")) setOpenMenuKey("");
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [openMenuKey]);

  const visibleItems = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return query
      ? items.filter((item) => item.title.toLocaleLowerCase().includes(query))
      : items;
  }, [items, search]);
  const allVisibleSelected = visibleItems.length > 0
    && visibleItems.every((item) => selectedKeys.has(itemKey(item)));

  function toggleAllVisible() {
    setSelectedKeys((current) => {
      const next = new Set(current);
      for (const item of visibleItems) {
        if (allVisibleSelected) next.delete(itemKey(item));
        else next.add(itemKey(item));
      }
      return next;
    });
  }

  function toggleItemSelection(item: DeletedItem) {
    setSelectedKeys((current) => {
      const next = new Set(current);
      const key = itemKey(item);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  async function restore(item: DeletedItem) {
    if (actionPending) return;
    setActionPending(true);
    setError("");
    setOpenMenuKey("");
    try {
      await recentlyDeletedClient.restore(item.id, item.itemType);
      setItems((current) => current.filter(
        (candidate) => candidate.id !== item.id || candidate.itemType !== item.itemType,
      ));
      setSelectedKeys((current) => {
        const next = new Set(current);
        next.delete(itemKey(item));
        return next;
      });
      showToast({ type: "success", message: "Item restored" });
    } catch {
      setError("KeyNest could not restore this item.");
    } finally {
      setActionPending(false);
    }
  }

  async function confirmPermanentDelete() {
    if (!confirming || actionPending) return false;
    setActionPending(true);
    setActionError("");
    try {
      if (confirming === "all") {
        await recentlyDeletedClient.empty();
        setItems([]);
        setSelectedKeys(new Set());
        showToast({ type: "success", message: "Recently Deleted emptied" });
      } else {
        await recentlyDeletedClient.permanentlyDelete(confirming.id, confirming.itemType);
        setItems((current) => current.filter(
          (candidate) => candidate.id !== confirming.id || candidate.itemType !== confirming.itemType,
        ));
        setSelectedKeys((current) => {
          const next = new Set(current);
          next.delete(itemKey(confirming));
          return next;
        });
        showToast({ type: "error", message: "Item permanently deleted" });
      }
      return true;
    } catch {
      setActionError("KeyNest could not permanently delete the selected item(s).");
      return false;
    } finally {
      setActionPending(false);
    }
  }

  return (
    <main className="password-vault-page recently-deleted-page">
      <header className="recently-deleted-heading">
        <div>
          <p className="eyebrow">QUICK ACCESS</p>
          <h1>Recently Deleted</h1>
          <p>Items are permanently deleted after 30 days.</p>
        </div>
      </header>

      <div className="recently-deleted-toolbar">
        <label className="recently-deleted-search">
          <Search size={17} aria-hidden="true" />
          <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search deleted items..." />
        </label>
        {items.length > 0 ? (
          <div className="recently-deleted-toolbar-actions">
            <button className="secondary-button recently-deleted-select-button" type="button" onClick={toggleAllVisible} disabled={visibleItems.length === 0}>
              {allVisibleSelected ? "Deselect all" : "Select all"}
            </button>
            <button className="vault-danger-button recently-deleted-empty-button" type="button" onClick={() => { setActionError(""); setConfirming("all"); }}>
              Empty Bin
            </button>
          </div>
        ) : null}
      </div>

      {loading ? <p className="vault-status" role="status">Loading deleted items...</p> : null}
      {error ? (
        <section className="vault-load-error" aria-live="assertive">
          <p>{error}</p>
          <button className="secondary-button" type="button" onClick={() => void loadItems()}>Retry</button>
        </section>
      ) : null}
      {!loading && !error && items.length === 0 ? (
        <section className="recently-deleted-empty-state">
          <Trash2 size={22} aria-hidden="true" />
          <h2>Recently Deleted is empty</h2>
          <p>Deleted items will appear here for 30 days.</p>
        </section>
      ) : null}
      {!loading && !error && items.length > 0 && visibleItems.length === 0 ? (
        <p className="vault-status">No matching deleted items</p>
      ) : null}
      {!loading && !error && visibleItems.length > 0 ? (
        <div className="recently-deleted-list" role="list">
          {visibleItems.map((item) => {
            const key = itemKey(item);
            const exactDate = deletedDateTime(item.deletedAtMs);
            const selected = selectedKeys.has(key);
            return (
              <article className={`recently-deleted-row${selected ? " selected" : ""}`} role="listitem" key={key}>
                <div className="recently-deleted-item-main">
                  <input
                    className="recently-deleted-row-select"
                    type="checkbox"
                    checked={selected}
                    aria-label={`Select ${item.title}`}
                    onChange={() => toggleItemSelection(item)}
                  />
                  <div className="recently-deleted-item-copy">
                    <strong>{item.title}</strong>
                    <span>{item.itemType === "credential" ? "Credential" : "Note"}</span>
                  </div>
                </div>
                <time dateTime={exactDate.dateTime} title={exactDate.title}>
                  {deletedDate(item.deletedAtMs)}
                </time>
                <div className="recently-deleted-row-actions">
                  <button className="recently-deleted-menu-button" type="button" aria-label={`More actions for ${item.title}`} aria-expanded={openMenuKey === key} onClick={() => setOpenMenuKey((current) => current === key ? "" : key)} disabled={actionPending}>
                    <MoreHorizontal size={18} aria-hidden="true" />
                  </button>
                  {openMenuKey === key ? (
                    <div className="recently-deleted-menu" role="menu">
                      <button type="button" role="menuitem" onClick={() => void restore(item)}>Restore</button>
                      <button className="danger" type="button" role="menuitem" onClick={() => { setOpenMenuKey(""); setActionError(""); setConfirming(item); }}>Delete permanently</button>
                    </div>
                  ) : null}
                </div>
              </article>
            );
          })}
        </div>
      ) : null}

      {confirming ? (
        <ConfirmPermanentDeleteDialog
          item={confirming === "all" ? null : confirming}
          pending={actionPending}
          error={actionError}
          onCancel={() => { if (!actionPending) setConfirming(null); }}
          onConfirm={confirmPermanentDelete}
        />
      ) : null}
    </main>
  );
}
