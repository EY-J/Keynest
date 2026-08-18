import { type ReactNode, useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { authClient } from "../authClient";
import { AuthClientError, type AuthStatus } from "../types";
import AuthLayout from "./AuthLayout";
import DataErrorScreen from "./DataErrorScreen";
import SetupScreen from "./SetupScreen";
import UnlockScreen from "./UnlockScreen";

type GateState = "checking" | AuthStatus;

type AuthGateProps = {
  children: (controls: {
    lock: () => Promise<void>;
    resetAuthenticated: (
      currentPassword: string,
      confirmation: "RESET KEYNEST",
    ) => Promise<void>;
  }) => ReactNode;
  onResetComplete?: () => void;
};

export default function AuthGate({ children, onResetComplete }: AuthGateProps) {
  const [status, setStatus] = useState<GateState>("checking");
  const [lockError, setLockError] = useState("");
  const [hasLockListener, setHasLockListener] = useState(false);

  const refreshStatus = useCallback(async () => {
    setStatus("checking");
    try {
      const nextStatus = await authClient.getStatus();
      if (isAuthStatus(nextStatus)) {
        setHasLockListener(false);
        setStatus(nextStatus);
      } else {
        setStatus("data-error");
      }
    } catch {
      setStatus("data-error");
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (status !== "unlocked") {
      return;
    }

    let isCurrent = true;
    let unlisten: (() => void) | undefined;
    setHasLockListener(false);

    void listen("keynest://locked", () => {
      if (!isCurrent) {
        return;
      }
      setLockError("");
      setStatus("locked");
    })
      .then((nextUnlisten) => {
        if (!isCurrent) {
          nextUnlisten();
          return;
        }
        unlisten = nextUnlisten;
        return authClient.getStatus().then(
          (nextStatus) => {
            if (!isCurrent) {
              return;
            }
            if (nextStatus === "unlocked") {
              setHasLockListener(true);
              return;
            }
            setStatus(isAuthStatus(nextStatus) ? nextStatus : "data-error");
          },
          () => {
            if (isCurrent) {
              setStatus("data-error");
            }
          },
        );
      })
      .catch(async () => {
        if (!isCurrent) {
          return;
        }
        try {
          const nextStatus = await authClient.lock();
          if (!isCurrent) {
            return;
          }
          setStatus(nextStatus === "locked" ? "locked" : "data-error");
        } catch {
          if (isCurrent) {
            setStatus("data-error");
          }
        }
      });

    return () => {
      isCurrent = false;
      unlisten?.();
    };
  }, [status]);

  async function reset(confirmation: string) {
    const nextStatus = await authClient.resetKeynest(confirmation);
    if (nextStatus !== "setup-required") {
      throw new AuthClientError(
        "unexpected-status",
        "KeyNest could not confirm that local data was reset.",
      );
    }
    onResetComplete?.();
    setHasLockListener(false);
    setStatus("setup-required");
  }

  async function resetAuthenticated(
    currentPassword: string,
    confirmation: "RESET KEYNEST",
  ) {
    const nextStatus = await authClient.resetKeynestAuthenticated(
      currentPassword,
      confirmation,
    );
    if (nextStatus !== "setup-required") {
      throw new AuthClientError(
        "unexpected-status",
        "KeyNest could not confirm that local data was reset.",
      );
    }
    onResetComplete?.();
    setHasLockListener(false);
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
    } catch {
      setLockError("KeyNest could not confirm that the vault was locked.");
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
      return (
        <SetupScreen
          onCreated={() => {
            setHasLockListener(false);
            setStatus("unlocked");
          }}
        />
      );
    case "locked":
      return (
        <UnlockScreen
          onUnlocked={() => {
            setHasLockListener(false);
            setStatus("unlocked");
          }}
          onReset={reset}
        />
      );
    case "data-error":
      return <DataErrorScreen onRetry={refreshStatus} onReset={reset} />;
    case "unlocked":
      if (!hasLockListener) {
        return (
          <AuthLayout
            eyebrow="KEYNEST SECURITY"
            title="Securing your nestâ€¦"
            description="Connecting secure lock controls on this device."
          >
            <div className="auth-loading" aria-label="Checking security status" />
          </AuthLayout>
        );
      }
      return (
        <>
          {lockError ? (
            <div className="lock-error-banner" role="alert">
              {lockError}
            </div>
          ) : null}
          {children({ lock, resetAuthenticated })}
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
