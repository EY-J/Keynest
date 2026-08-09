type NavigationSidebarProps = {
  isOpen: boolean;
  onClose: () => void;
  onOpenPasswordVault: () => void;
};

export default function NavigationSidebar({
  isOpen,
  onClose,
  onOpenPasswordVault,
}: NavigationSidebarProps) {
  function openPasswordVault() {
    onOpenPasswordVault();
    onClose();
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

        <button className="sidebar-link active" type="button" onClick={onClose}>
          <span className="sidebar-link-icon" aria-hidden="true">
            ⌂
          </span>

          <span>Home</span>
        </button>

        <button
          className="sidebar-link"
          type="button"
          onClick={openPasswordVault}
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
        <button className="sidebar-link" type="button">
          <span className="sidebar-link-icon" aria-hidden="true">
            ⚙
          </span>

          <span>Settings</span>
        </button>

        <button className="sidebar-link sidebar-lock-button" type="button">
          <span className="sidebar-link-icon" aria-hidden="true">
            ↪
          </span>

          <span>Lock KeyNest</span>
        </button>
      </div>
    </aside>
  );
}