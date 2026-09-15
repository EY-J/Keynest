import {
  Folder,
  House,
  KeyRound,
  Lock,
  NotebookPen,
  Settings,
  Sparkles,
  Star,
  Trash2,
} from "lucide-react";
import type { AuthenticatedDestination } from "../AppShell";
import ProfileAvatar from "../../features/profile/ProfileAvatar";
import type { Profile } from "../../features/profile/profileTypes";

type NavigationSidebarProps = {
  isOpen: boolean;
  activeDestination: AuthenticatedDestination;
  onClose(): void;
  onNavigate(destination: AuthenticatedDestination): void;
  onLockKeynest(): Promise<void>;
  profile: Profile;
};

export default function NavigationSidebar({
  isOpen,
  activeDestination,
  onClose,
  onNavigate,
  onLockKeynest,
  profile,
}: NavigationSidebarProps) {
  function lockKeynest() {
    onClose();
    void onLockKeynest();
  }

  return (
    <aside
      id="keynest-sidebar"
      className={`navigation-sidebar ${
        isOpen ? "navigation-sidebar-open" : ""
      }`}
      aria-hidden={!isOpen}
    >
      <div className="sidebar-profile">
        <ProfileAvatar avatarUrl={profile.avatarDataUrl} className="sidebar-avatar" />
        <div className="sidebar-profile-details">
          <strong>{profile.isConfigured ? profile.displayName : "Local account"}</strong>
          <span>{profile.isConfigured ? "Local account" : "Display name not set"}</span>
        </div>
      </div>

      <nav className="sidebar-navigation" aria-label="KeyNest navigation">
        <section className="sidebar-group" aria-labelledby="sidebar-library-title">
          <h2 className="sidebar-section-title" id="sidebar-library-title">
            Library
          </h2>

          <button
            className={`sidebar-link ${
              activeDestination === "home" ? "active" : ""
            }`}
            type="button"
            aria-current={activeDestination === "home" ? "page" : undefined}
            onClick={() => onNavigate("home")}
          >
            <House className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Home</span>
          </button>

          <button
            className={`sidebar-link ${
              activeDestination === "vault" ? "active" : ""
            }`}
            type="button"
            aria-current={activeDestination === "vault" ? "page" : undefined}
            onClick={() => onNavigate("vault")}
          >
            <KeyRound className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Vault</span>
          </button>

          <button
            className={`sidebar-link ${activeDestination === "notes" ? "active" : ""}`}
            type="button"
            aria-current={activeDestination === "notes" ? "page" : undefined}
            onClick={() => onNavigate("notes")}
          >
            <NotebookPen className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Notes</span>
          </button>

          <button className="sidebar-link" type="button" disabled>
            <Sparkles className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Placeholder</span>
            <span className="sidebar-badge">Soon</span>
          </button>
        </section>

        <section
          className="sidebar-group"
          aria-labelledby="sidebar-quick-access-title"
        >
          <h2 className="sidebar-section-title" id="sidebar-quick-access-title">
            Quick Access
          </h2>

          <button
            className={`sidebar-link ${
              activeDestination === "favorites" ? "active" : ""
            }`}
            type="button"
            aria-current={activeDestination === "favorites" ? "page" : undefined}
            onClick={() => onNavigate("favorites")}
          >
            <Star className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Favorites</span>
          </button>

          <button className="sidebar-link" type="button" disabled>
            <Folder className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Folders</span>
            <span className="sidebar-badge">Soon</span>
          </button>

          <button
            className={`sidebar-link ${
              activeDestination === "recently-deleted" ? "active" : ""
            }`}
            type="button"
            aria-current={activeDestination === "recently-deleted" ? "page" : undefined}
            onClick={() => onNavigate("recently-deleted")}
          >
            <Trash2 className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Recently Deleted</span>
          </button>
        </section>
      </nav>

      <div className="sidebar-footer">
        <button
          className={`sidebar-link ${
            activeDestination === "settings" ? "active" : ""
          }`}
          type="button"
          aria-current={activeDestination === "settings" ? "page" : undefined}
          onClick={() => onNavigate("settings")}
        >
          <Settings className="sidebar-link-icon" size={20} aria-hidden="true" />
          <span className="sidebar-link-label">Settings</span>
        </button>

        <button
          className="sidebar-link sidebar-lock-button"
          title="Lock KeyNest (Ctrl+Shift+L)"
          aria-keyshortcuts="Control+Shift+L"
          type="button"
          onClick={lockKeynest}
        >
          <Lock className="sidebar-link-icon" size={20} aria-hidden="true" />
          <span className="sidebar-link-label">Lock KeyNest</span>
        </button>
      </div>
    </aside>
  );
}
