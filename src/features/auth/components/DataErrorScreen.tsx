import { useState } from "react";
import AuthLayout from "./AuthLayout";
import ResetDialog from "./ResetDialog";

type DataErrorScreenProps = {
  onRetry: () => Promise<void>;
  onReset: (confirmation: string) => Promise<void>;
};

export default function DataErrorScreen({
  onRetry,
  onReset,
}: DataErrorScreenProps) {
  const [isResetOpen, setIsResetOpen] = useState(false);
  const [isRetrying, setIsRetrying] = useState(false);

  async function retry() {
    setIsRetrying(true);
    try {
      await onRetry();
    } finally {
      setIsRetrying(false);
    }
  }

  return (
    <AuthLayout
      eyebrow="LOCAL DATA ERROR"
      title="KeyNest could not verify your local data."
      description={
        "The encrypted profile is damaged, incomplete, unsupported, or currently " +
        "unavailable. KeyNest stayed locked."
      }
    >
      <div className="data-error-actions">
        <button
          className="primary-button"
          disabled={isRetrying}
          onClick={() => void retry()}
        >
          {isRetrying ? "Checking…" : "Try again"}
        </button>
        <button
          className="auth-reset-link danger-text"
          type="button"
          onClick={() => setIsResetOpen(true)}
        >
          Reset KeyNest
        </button>
      </div>

      <ResetDialog
        isOpen={isResetOpen}
        onClose={() => setIsResetOpen(false)}
        onReset={onReset}
      />
    </AuthLayout>
  );
}
