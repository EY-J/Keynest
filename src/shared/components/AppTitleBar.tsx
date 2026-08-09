import { getCurrentWindow } from "@tauri-apps/api/window";

type AppTitleBarProps = {
  isNavigationOpen?: boolean;
  onOpenNavigation?: () => void;
};

export default function AppTitleBar({
  isNavigationOpen = false,
  onOpenNavigation,
}: AppTitleBarProps) {
  const appWindow = getAppWindow();

  return (
    <div className="app-titlebar" data-tauri-drag-region>
      {onOpenNavigation ? (
        <button
          className="titlebar-menu-button"
          type="button"
          aria-label="Open navigation"
          aria-controls="keynest-sidebar"
          aria-expanded={isNavigationOpen}
          onClick={onOpenNavigation}
        >
          <span />
          <span />
          <span />
        </button>
      ) : (
        <div className="titlebar-menu-placeholder" aria-hidden="true" />
      )}

      <div
        className="titlebar-app-name"
        data-tauri-drag-region
        onDoubleClick={() => void appWindow?.toggleMaximize()}
      >
        <span data-tauri-drag-region>KeyNest</span>
      </div>

      <div className="titlebar-window-controls">
        <button
          type="button"
          aria-label="Minimize window"
          onClick={() => void appWindow?.minimize()}
        >
          <span className="window-minimize" />
        </button>
        <button
          type="button"
          aria-label="Maximize window"
          onClick={() => void appWindow?.toggleMaximize()}
        >
          <span className="window-maximize" />
        </button>
        <button
          className="window-close-button"
          type="button"
          aria-label="Close window"
          onClick={() => void appWindow?.close()}
        >
          <span className="window-close" />
        </button>
      </div>
    </div>
  );
}

function getAppWindow() {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}
