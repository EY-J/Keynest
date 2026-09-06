import {
  FolderLock,
  House,
  KeyRound,
  Lock,
  NotebookPen,
  Settings,
  Star,
} from "lucide-react";

type NavigationSidebarProps = {
  isOpen: boolean;
  activeDestination: "home" | "vault" | "settings";
  onClose(): void;
  onNavigate(destination: "home" | "vault" | "settings"): void;
  onLockKeynest(): Promise<void>;
};

export default function NavigationSidebar({
  isOpen,
  activeDestination,
  onClose,
  onNavigate,
  onLockKeynest,
}: NavigationSidebarProps) {
  function navigate(destination: "home" | "vault" | "settings") {
    onNavigate(destination);
  }

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
        <div className="sidebar-avatar">AJ</div>

        <div className="sidebar-profile-details">
          <strong>KeyNest User</strong>
          <span>Local account</span>
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
            onClick={() => navigate("home")}
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
            onClick={() => navigate("vault")}
          >
            <KeyRound className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Password Vault</span>
          </button>

          <button className="sidebar-link" type="button" disabled>
            <NotebookPen className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Secure Notes</span>
            <span className="sidebar-badge">Soon</span>
          </button>

          <button className="sidebar-link" type="button" disabled>
            <FolderLock className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Private Files</span>
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

          <button className="sidebar-link" type="button">
            <Star className="sidebar-link-icon" size={20} aria-hidden="true" />
            <span className="sidebar-link-label">Favorites</span>
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
          onClick={() => navigate("settings")}
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
