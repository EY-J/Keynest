import "../App.css";
import AuthGate from "../features/auth/components/AuthGate";
import HomePage from "../pages/HomePage";

export default function App() {
  function openPasswordVault() {
    alert("The Password Vault page will open here.");
  }

  return (
    <AuthGate>
      {({ lock }) => (
        <HomePage
          onOpenPasswordVault={openPasswordVault}
          onLockKeynest={lock}
        />
      )}
    </AuthGate>
  );
}
