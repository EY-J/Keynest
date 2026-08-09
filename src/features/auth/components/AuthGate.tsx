import { type ReactNode, useCallback, useEffect, useState } from "react";
import { authClient } from "../authClient";
import { AuthClientError, type AuthStatus } from "../types";
import AuthLayout from "./AuthLayout";
import DataErrorScreen from "./DataErrorScreen";
import SetupScreen from "./SetupScreen";
import UnlockScreen from "./UnlockScreen";

type GateState = "checking" | AuthStatus;

type AuthGateProps = {
  children: (controls: { lock: () => Promise<void> }) => ReactNode;
};

export default function AuthGate({ children }: AuthGateProps) {
  const [status, setStatus] = useState<GateState>("checking");
  const [lockError, setLockError] = useState("");

  const refreshStatus = useCallback(async () => {
    setStatus("checking");
    try {
      const nextStatus = await authClient.getStatus();
      setStatus(isAuthStatus(nextStatus) ? nextStatus : "data-error");
    } catch {
      setStatus("data-error");
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  async function reset(confirmation: string) {
    const nextStatus = await authClient.resetKeynest(confirmation);
    if (nextStatus !== "setup-required") {
      throw new AuthClientError(
        "unexpected-status",
        "KeyNest could not confirm that local data was reset.",
      );
    }
    setStatus("setup-required");
  }

  async function lock() {
    setLockError("");
    try {
      const nextStatus = await authClient.lock();
      if (nextStatus !== "locked") {
        setLockError("KeyNest could not confirm that the vault was locked.");
        return;
      }
      setStatus("locked");
    } catch (error) {
      setLockError(
        error instanceof Error
          ? error.message
          : "KeyNest could not confirm that the vault was locked.",
      );
    }
  }

  switch (status) {
    case "checking":
      return (
        <AuthLayout
          eyebrow="KEYNEST SECURITY"
          title="Securing your nest…"
          description="Checking the encrypted profile on this device."
        >
          <div className="auth-loading" aria-label="Checking security status" />
        </AuthLayout>
      );
    case "setup-required":
      return <SetupScreen onCreated={() => setStatus("unlocked")} />;
    case "locked":
      return (
        <UnlockScreen
          onUnlocked={() => setStatus("unlocked")}
          onReset={reset}
        />
      );
    case "data-error":
      return <DataErrorScreen onRetry={refreshStatus} onReset={reset} />;
    case "unlocked":
      return (
        <>
          {lockError ? (
            <div className="lock-error-banner" role="alert">
              {lockError}
            </div>
          ) : null}
          {children({ lock })}
        </>
      );
  }
}

function isAuthStatus(value: unknown): value is AuthStatus {
  return (
    value === "setup-required" ||
    value === "locked" ||
    value === "unlocked" ||
    value === "data-error"
  );
}
