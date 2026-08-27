import { useEffect, useState } from "react";
import { useSettings } from "../../features/settings/SettingsProvider";
import HomePage from "../../pages/HomePage";
import PasswordVaultPage from "../../pages/PasswordVaultPage";
import SettingsPage from "../../pages/SettingsPage";
import AppTitleBar from "./AppTitleBar";
import NavigationSidebar from "./NavigationSidebar";

export type AuthenticatedDestination = "home" | "vault" | "settings";

type AuthenticatedShellProps = {
  onLockKeynest: () => Promise<void>;
  onResetAuthenticated: (
    currentPassword: string,
    confirmation: "RESET KEYNEST",
  ) => Promise<void>;
};

export default function AuthenticatedShell({
  onLockKeynest,
  onResetAuthenticated,
}: AuthenticatedShellProps) {
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [activeDestination, setActiveDestination] =
    useState<AuthenticatedDestination>("home");
  const { settings } = useSettings();

  useEffect(() => {
    if (!isSidebarOpen) {
      return;
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setIsSidebarOpen(false);
      }
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [isSidebarOpen]);

  function navigate(destination: AuthenticatedDestination) {
    setActiveDestination(destination);
    setIsSidebarOpen(false);
  }

  return (
    <div className="home-page">
      <AppTitleBar
        isNavigationOpen={isSidebarOpen}
        onOpenNavigation={() => setIsSidebarOpen((isOpen) => !isOpen)}
      />

      <NavigationSidebar
        isOpen={isSidebarOpen}
        activeDestination={activeDestination}
        onClose={() => setIsSidebarOpen(false)}
        onNavigate={navigate}
        onLockKeynest={onLockKeynest}
      />

      <button
        className={`sidebar-backdrop ${isSidebarOpen ? "visible" : ""}`}
        type="button"
        aria-label="Close navigation"
        tabIndex={isSidebarOpen ? 0 : -1}
        onClick={() => setIsSidebarOpen(false)}
      />

      {settings.warning ? (
        <p className="settings-warning-banner" role="status">
          {settings.warning}
        </p>
      ) : null}

      {activeDestination === "home" ? (
        <HomePage onNavigateToVault={() => navigate("vault")} />
      ) : activeDestination === "vault" ? (
        <PasswordVaultPage />
      ) : (
        <SettingsPage onResetAuthenticated={onResetAuthenticated} />
      )}
    </div>
  );
}
