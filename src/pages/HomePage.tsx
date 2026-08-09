import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import NavigationSidebar from "../shared/components/NavigationSidebar";

type HomePageProps = {
  onOpenPasswordVault: () => void;
};

export default function HomePage({
  onOpenPasswordVault,
}: HomePageProps) {
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

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

  const appWindow = getCurrentWindow();

  return (
    <div className="home-page">
      <div
        className="app-titlebar"
        data-tauri-drag-region
      >
        <button
          className="titlebar-menu-button"
          type="button"
          aria-label="Open navigation"
          aria-controls="keynest-sidebar"
          aria-expanded={isSidebarOpen}
          onClick={() => setIsSidebarOpen((isOpen) => !isOpen)}
        >
          <span />
          <span />
          <span />
        </button>

        <div
          className="titlebar-app-name"
          data-tauri-drag-region
          onDoubleClick={() => void appWindow.toggleMaximize()}
        >
          <span data-tauri-drag-region>KeyNest</span>
        </div>

        <div className="titlebar-window-controls">
          <button
            type="button"
            aria-label="Minimize window"
            onClick={() => void appWindow.minimize()}
          >
            <span className="window-minimize" />
          </button>
          <button
            type="button"
            aria-label="Maximize window"
            onClick={() => void appWindow.toggleMaximize()}
          >
            <span className="window-maximize" />
          </button>
          <button
            className="window-close-button"
            type="button"
            aria-label="Close window"
            onClick={() => void appWindow.close()}
          >
            <span className="window-close" />
          </button>
        </div>
      </div>

      <NavigationSidebar
        isOpen={isSidebarOpen}
        onClose={() => setIsSidebarOpen(false)}
        onOpenPasswordVault={onOpenPasswordVault}
      />

      <button
        className={`sidebar-backdrop ${isSidebarOpen ? "visible" : ""}`}
        type="button"
        aria-label="Close navigation"
        tabIndex={isSidebarOpen ? 0 : -1}
        onClick={() => setIsSidebarOpen(false)}
      />

      <header className="topbar">
        <div className="brand">
          <div className="logo">K</div>

          <div>
            <h2>KeyNest</h2>
            <p>Your private digital space</p>
          </div>
        </div>

        <button className="profile-button" type="button">
          AJ
        </button>
      </header>

      <main className="home-content">
        <section className="hero">
          <p className="eyebrow">LOCAL WINDOWS APPLICATION</p>

          <h1>
            Keep your important information inside your
            <span> secure nest.</span>
          </h1>

          <p className="hero-description">
            KeyNest is a private Windows application that stores information
            locally on your device. Start with the Password Vault and add more
            useful features later.
          </p>

          <button
            className="primary-button"
            type="button"
            onClick={onOpenPasswordVault}
          >
            Open Password Vault
          </button>
        </section>

        <section className="features">
          <div className="section-heading">
            <p>KEYNEST FEATURES</p>
            <h2>Choose a feature</h2>
          </div>

          <div className="feature-grid">
            <article className="feature-card active-feature">
              <div className="feature-icon">🔑</div>

              <span className="status available">Available</span>

              <h3>Password Vault</h3>

              <p>
                Store and organize account usernames, passwords, websites, and
                private notes.
              </p>

              <button
                className="feature-button"
                type="button"
                onClick={onOpenPasswordVault}
              >
                Open Vault
                <span>→</span>
              </button>
            </article>

            <article className="feature-card">
              <div className="feature-icon">📝</div>

              <span className="status">Coming soon</span>

              <h3>Secure Notes</h3>

              <p>
                Store recovery codes, private notes, and other sensitive
                information.
              </p>

              <button className="feature-button" type="button" disabled>
                Not Available
              </button>
            </article>

            <article className="feature-card">
              <div className="feature-icon">📁</div>

              <span className="status">Coming soon</span>

              <h3>Private Files</h3>

              <p>
                A possible future feature for protecting important personal
                files.
              </p>

              <button className="feature-button" type="button" disabled>
                Not Available
              </button>
            </article>
          </div>
        </section>
      </main>

      <footer>
        <span>KeyNest</span>
        <span>Local-first privacy for Windows</span>
      </footer>
    </div>
  );
}
