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

        <div>
          <strong>KeyNest User</strong>
          <span>Local account</span>
        </div>
      </div>

      <nav className="sidebar-navigation" aria-label="KeyNest navigation">
        <p className="sidebar-section-title">Main</p>

        <button
          className={`sidebar-link ${
            activeDestination === "home" ? "active" : ""
          }`}
          type="button"
          onClick={() => navigate("home")}
        >
          <span className="sidebar-link-icon" aria-hidden="true">
            ⌂
          </span>

          <span>Home</span>
        </button>

        <button
          className={`sidebar-link ${
            activeDestination === "vault" ? "active" : ""
          }`}
          type="button"
          onClick={() => navigate("vault")}
        >
          <span className="sidebar-link-icon" aria-hidden="true">
            🔑
          </span>

          <span>Password Vault</span>
        </button>

        <button className="sidebar-link" type="button" disabled>
          <span className="sidebar-link-icon" aria-hidden="true">
            📝
          </span>

          <span>Secure Notes</span>
          <span className="sidebar-badge">Soon</span>
        </button>

        <button className="sidebar-link" type="button" disabled>
          <span className="sidebar-link-icon" aria-hidden="true">
            📁
          </span>

          <span>Private Files</span>
          <span className="sidebar-badge">Soon</span>
        </button>

        <p className="sidebar-section-title">Tools</p>

        <button className="sidebar-link" type="button">
          <span className="sidebar-link-icon" aria-hidden="true">
            ✦
          </span>

          <span>Password Generator</span>
        </button>

        <button className="sidebar-link" type="button">
          <span className="sidebar-link-icon" aria-hidden="true">
            ★
          </span>

          <span>Favorites</span>
        </button>
      </nav>

      <div className="sidebar-footer">
        <button
          className={`sidebar-link ${
            activeDestination === "settings" ? "active" : ""
          }`}
          type="button"
          onClick={() => navigate("settings")}
        >
          <span className="sidebar-link-icon" aria-hidden="true">
            ⚙
          </span>

          <span>Settings</span>
        </button>

        <button
          className="sidebar-link sidebar-lock-button"
          type="button"
          onClick={lockKeynest}
        >
          <span className="sidebar-link-icon" aria-hidden="true">
            ↪
          </span>

          <span>Lock KeyNest</span>
        </button>
      </div>
    </aside>
  );
}
