import "../App.css";
import { useState } from "react";
import AuthGate from "../features/auth/components/AuthGate";
import ActivityReporter from "../features/settings/ActivityReporter";
import SettingsProvider from "../features/settings/SettingsProvider";
import AuthenticatedShell from "../shared/components/AuthenticatedShell";

export default function App() {
  return (
    <SettingsProvider>
      <KeyNestApp />
    </SettingsProvider>
  );
}

function KeyNestApp() {
  const [activityError, setActivityError] = useState("");

  function openPasswordVault() {
    alert("The Password Vault page will open here.");
  }

  return (
    <AuthGate>
      {({ lock }) => (
        <>
          <ActivityReporter onError={setActivityError} />
          {activityError ? (
            <p className="settings-warning-banner" role="alert">
              {activityError}
            </p>
          ) : null}
          <AuthenticatedShell
            onOpenPasswordVault={openPasswordVault}
            onLockKeynest={lock}
          />
        </>
      )}
    </AuthGate>
  );
}
