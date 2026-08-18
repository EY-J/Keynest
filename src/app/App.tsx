import "../App.css";
import AuthGate from "../features/auth/components/AuthGate";
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
  function openPasswordVault() {
    alert("The Password Vault page will open here.");
  }

  return (
    <AuthGate>
      {({ lock }) => (
        <AuthenticatedShell
          onOpenPasswordVault={openPasswordVault}
          onLockKeynest={lock}
        />
      )}
    </AuthGate>
  );
}
