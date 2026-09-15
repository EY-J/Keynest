import "../styles/globals.css";
import { useState } from "react";
import AuthGate from "../features/auth/components/AuthGate";
import ActivityReporter from "../features/settings/components/ActivityReporter";
import SettingsProvider, { useSettings } from "../features/settings/SettingsProvider";
import AppShell from "./AppShell";
import { useScrollActivity } from "../hooks/useScrollActivity";
import { ToastProvider } from "../components/ui/Toast/ToastProvider";

export default function App() {
  return (
    <SettingsProvider>
      <ToastProvider>
        <KeyNestApp />
      </ToastProvider>
    </SettingsProvider>
  );
}

function KeyNestApp() {
  const [activityError, setActivityError] = useState("");
  const { resetToDefaults } = useSettings();
  useScrollActivity();

  return (
    <AuthGate onResetComplete={resetToDefaults}>
      {({ lock, resetAuthenticated }) => (
        <>
          <ActivityReporter onError={setActivityError} />
          {activityError ? (
            <p className="settings-warning-banner" role="alert">
              {activityError}
            </p>
          ) : null}
          <AppShell
            onLockKeynest={lock}
            onResetAuthenticated={resetAuthenticated}
          />
        </>
      )}
    </AuthGate>
  );
}
