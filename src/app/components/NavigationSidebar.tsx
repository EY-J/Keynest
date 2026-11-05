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
import { NavLink } from "react-router-dom";
import ProfileAvatar from "../../features/profile/ProfileAvatar";
import type { Profile } from "../../features/profile/profileTypes";

type NavigationSidebarProps = {
  isOpen: boolean;
  onClose(): void;
  onLockKeynest(): Promise<void>;
  profile: Profile;
};

export default function NavigationSidebar({
  isOpen,
  onClose,
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

          <NavLink
            className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
            to="/home"
            end
            onClick={onClose}
          >
            <House className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Home</span>
          </NavLink>

          <NavLink
            className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
            to="/vault"
            end
            onClick={onClose}
          >
            <KeyRound className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Vault</span>
          </NavLink>

          <NavLink
            className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
            to="/notes"
            end
            onClick={onClose}
          >
            <NotebookPen className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Notes</span>
          </NavLink>

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

          <NavLink
            className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
            to="/favorites"
            end
            onClick={onClose}
          >
            <Star className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Favorites</span>
          </NavLink>

          <NavLink className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`} to="/folders" end onClick={onClose}>
            <Folder className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Folders</span>
          </NavLink>

          <NavLink
            className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
            to="/recently-deleted"
            end
            onClick={onClose}
          >
            <Trash2 className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Recently Deleted</span>
          </NavLink>
        </section>
      </nav>

      <div className="sidebar-footer">
        <NavLink
          className={({ isActive }) => `sidebar-link${isActive ? " active" : ""}`}
          to="/settings"
          end
          onClick={onClose}
        >
          <Settings className="sidebar-link-icon" size={20} aria-hidden="true" />
          <span className="sidebar-link-label">Settings</span>
        </NavLink>

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
