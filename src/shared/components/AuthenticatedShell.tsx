import { useEffect, useState } from "react";
import { useSettings } from "../../features/settings/SettingsProvider";
import HostApprovalModal from "../../features/autofill/HostApprovalModal";
import {
  loadFavoriteRecordIds,
  saveFavoriteRecordIds,
  toggledFavoriteRecordIds,
} from "../../features/vault/favoriteStore";
import HomePage from "../../pages/HomePage";
import PasswordVaultPage from "../../pages/PasswordVaultPage";
import SettingsPage from "../../pages/SettingsPage";
import AppTitleBar from "./AppTitleBar";
import NavigationSidebar from "./NavigationSidebar";

export type AuthenticatedDestination = "home" | "vault" | "favorites" | "settings";

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
  const [favoriteRecordIds, setFavoriteRecordIds] = useState<Set<string>>(
    loadFavoriteRecordIds,
  );
  const { settings } = useSettings();

  useEffect(() => {
    saveFavoriteRecordIds(favoriteRecordIds);
  }, [favoriteRecordIds]);

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

  function toggleFavorite(recordId: string) {
    setFavoriteRecordIds((current) => toggledFavoriteRecordIds(current, recordId));
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
      ) : activeDestination === "vault" || activeDestination === "favorites" ? (
        <PasswordVaultPage
          favoriteRecordIds={favoriteRecordIds}
          favoritesOnly={activeDestination === "favorites"}
          onToggleFavorite={toggleFavorite}
        />
      ) : (
        <SettingsPage onResetAuthenticated={onResetAuthenticated} />
      )}
      <HostApprovalModal />
    </div>
  );
}
