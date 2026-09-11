import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import Modal, { ModalCloseButton, useModalClose } from "../../../components/ui/Modal/Modal";
import ServiceLogo from "../../../components/ui/ServiceLogo";
import {
  hostApprovalClient,
  type HostApprovalCandidate,
  type PendingHostApproval,
} from "../hostApprovalClient";

const HOST_APPROVAL_EVENT = "keynest://host-approval-requested";

export default function HostApprovalModal() {
  const [pending, setPending] = useState<PendingHostApproval | null>(null);
  const [candidates, setCandidates] = useState<HostApprovalCandidate[]>([]);
  const [selected, setSelected] = useState<HostApprovalCandidate | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [success, setSuccess] = useState("");
  const cancelRef = useRef<HTMLButtonElement>(null);
  const close = useModalClose(() => setPending(null));

  const loadPending = useCallback(async () => {
    try {
      const request = await hostApprovalClient.pending();
      setPending(request);
      setSelected(null);
      setCandidates([]);
      setError("");
      if (!request) return;
      setLoading(true);
      const available = await hostApprovalClient.candidates(request.approvalId);
      setCandidates(available);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "KeyNest could not load this approval request.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    const onFocus = () => { void loadPending(); };
    window.addEventListener("focus", onFocus);
    void listen(HOST_APPROVAL_EVENT, () => { void loadPending(); })
      .then(dispose => {
        if (!active) { dispose(); return; }
        unlisten = dispose;
        // Subscribe before the first read so a request cannot land between an
        // empty initial read and listener registration.
        void loadPending();
      })
      .catch(() => {
        // Window focus remains a safe state-based fallback if event setup is
        // transiently unavailable; no approval data comes from the event.
        if (active) void loadPending();
      });
    return () => {
      active = false;
      window.removeEventListener("focus", onFocus);
      unlisten?.();
    };
  }, [loadPending]);

  useEffect(() => {
    if (!success) return;
    const timer = window.setTimeout(() => setSuccess(""), 6000);
    return () => window.clearTimeout(timer);
  }, [success]);

  async function cancel() {
    if (!pending || saving || close.closing) return;
    setSaving(true);
    setError("");
    try {
      await hostApprovalClient.cancel(pending.approvalId);
      close.close(() => {
        setPending(null);
        setCandidates([]);
        setSelected(null);
      });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "KeyNest could not cancel this request.");
    } finally {
      setSaving(false);
    }
  }

  async function approve() {
    if (!pending || !selected || saving || close.closing) return;
    setSaving(true);
    setError("");
    try {
      const result = await hostApprovalClient.approve(pending.approvalId, selected.credentialId);
      close.close(() => {
        setPending(null);
        setCandidates([]);
        setSelected(null);
        setSuccess(`Login host approved. ${result.requestedHost} can now use the selected ${result.credentialName} credential.`);
      });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "KeyNest could not approve this login host.");
    } finally {
      setSaving(false);
    }
  }

  return <>
    {success ? <p className="host-approval-toast" role="status">{success}</p> : null}
    {pending ? (
      <Modal titleId="host-approval-title" className="host-approval-dialog" width={620}
        closing={close.closing} onClose={() => { void cancel(); }} onExitComplete={close.finishClose}
        pending={saving} closeOnBackdrop initialFocusRef={cancelRef}>
        <div className="host-approval-header">
          <div className="host-approval-heading">
            <div>
              <h2 id="host-approval-title">Approve login host</h2>
            </div>
            <ModalCloseButton onClick={() => { void cancel(); }} disabled={saving} label="Cancel host approval" />
          </div>
          <p className="host-approval-request-host" dir="ltr">{pending.requestedHost}</p>
          {!selected ? <p className="host-approval-copy">Choose a credential to use on this site.</p> : null}
        </div>

        {!selected ? <>
          {loading ? <p className="host-approval-muted" role="status">Loading saved credentials…</p> : null}
          {!loading && candidates.length === 0 ? <p className="host-approval-muted">No saved credentials are available.</p> : null}
          <div className="host-approval-candidates">
            {candidates.map(candidate => (
              <button className="host-approval-candidate" type="button" key={candidate.credentialId}
                onClick={() => setSelected(candidate)} disabled={saving}>
                <ServiceLogo name={candidate.name} website={candidate.website} size="small" />
                <span className="host-approval-candidate-copy"><strong>{candidate.name}</strong><small>{candidate.username}</small>
                  <small>Saved website: {candidate.website || "Not set"}</small></span>
                <span aria-hidden="true">›</span>
              </button>
            ))}
          </div>
        </> : <>
          <div className="host-approval-selected">
            <strong>{selected.name}</strong>
            <span>{selected.username}</span>
          </div>
          <dl className="host-approval-comparison">
            <div><dt>Saved website</dt><dd dir="ltr">{selected.website || "Not set"}</dd></div>
            <div><dt>Requested login host</dt><dd dir="ltr">{pending.requestedHost}</dd></div>
          </dl>
          <p className="host-approval-copy">Allow KeyNest Autofill to use this credential on that exact hostname. Only approve it if you recognize and trust this sign-in address.</p>
        </>}

        {error ? <p className="host-approval-error" role="alert">{error}</p> : null}
        <div className="host-approval-actions">
          {selected ? <button className="secondary-button" type="button" onClick={() => setSelected(null)} disabled={saving}>Back</button> : null}
          <button ref={cancelRef} className="secondary-button" type="button" onClick={() => { void cancel(); }} disabled={saving}>Cancel</button>
          {selected ? <button className="primary-button" type="button" onClick={() => { void approve(); }} disabled={saving}>
            {saving ? "Approving…" : "Allow Host"}
          </button> : null}
        </div>
      </Modal>
    ) : null}
  </>;
}
