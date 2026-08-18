import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import AuthLayout from "../auth/components/AuthLayout";
import { settingsClient } from "./settingsClient";
import type {
  AutoLockSeconds,
  ClipboardClearSeconds,
  SettingsSnapshot,
  ThemePreference,
} from "./types";

const LOAD_FAILURE_WARNING =
  "KeyNest could not load saved preferences. Secure defaults are active.";

const DEFAULT_SETTINGS: SettingsSnapshot = {
  autoLockSeconds: 300,
  clipboardClearSeconds: 30,
  theme: "system",
  launchAtStartup: false,
};

type SettingsContextValue = {
  settings: SettingsSnapshot;
  setAutoLockSeconds(value: AutoLockSeconds): Promise<void>;
  setClipboardClearSeconds(value: ClipboardClearSeconds): Promise<void>;
  setTheme(value: ThemePreference): Promise<void>;
  setLaunchAtStartup(enabled: boolean): Promise<void>;
  resetToDefaults(): void;
  reload(): Promise<void>;
};

const SettingsContext = createContext<SettingsContextValue | null>(null);

type SettingsProviderProps = {
  children: ReactNode;
};

export default function SettingsProvider({ children }: SettingsProviderProps) {
  const [settings, setSettings] = useState<SettingsSnapshot>(DEFAULT_SETTINGS);
  const [hasLoaded, setHasLoaded] = useState(false);

  const reload = useCallback(async () => {
    try {
      setSettings(await settingsClient.getSettings());
    } catch {
      setSettings({ ...DEFAULT_SETTINGS, warning: LOAD_FAILURE_WARNING });
    } finally {
      setHasLoaded(true);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    const root = document.documentElement;

    function applyTheme(resolvedTheme: "dark" | "light") {
      root.dataset.theme = resolvedTheme;
      root.style.colorScheme = resolvedTheme;
    }

    if (settings.theme !== "system") {
      applyTheme(settings.theme);
      return;
    }

    if (typeof window.matchMedia !== "function") {
      applyTheme("light");
      return;
    }

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const followSystemTheme = (event: MediaQueryListEvent) => {
      applyTheme(event.matches ? "dark" : "light");
    };

    applyTheme(mediaQuery.matches ? "dark" : "light");
    mediaQuery.addEventListener("change", followSystemTheme);
    return () => mediaQuery.removeEventListener("change", followSystemTheme);
  }, [settings.theme]);

  const setAutoLockSeconds = useCallback(async (value: AutoLockSeconds) => {
    setSettings(await settingsClient.setAutoLockSeconds(value));
  }, []);

  const setClipboardClearSeconds = useCallback(
    async (value: ClipboardClearSeconds) => {
      setSettings(await settingsClient.setClipboardClearSeconds(value));
    },
    [],
  );

  const setTheme = useCallback(async (value: ThemePreference) => {
    setSettings(await settingsClient.setTheme(value));
  }, []);

  const setLaunchAtStartup = useCallback(async (enabled: boolean) => {
    setSettings(await settingsClient.setLaunchAtStartup(enabled));
  }, []);

  const resetToDefaults = useCallback(() => {
    setSettings(DEFAULT_SETTINGS);
  }, []);

  const contextValue: SettingsContextValue = {
    settings,
    setAutoLockSeconds,
    setClipboardClearSeconds,
    setTheme,
    setLaunchAtStartup,
    resetToDefaults,
    reload,
  };

  if (!hasLoaded) {
    return (
      <AuthLayout
        eyebrow="KEYNEST SETTINGS"
        title="Preparing your nest…"
        description="Loading preferences for this device."
      >
        <div className="auth-loading" aria-label="Checking saved preferences" />
      </AuthLayout>
    );
  }

  return (
    <SettingsContext.Provider value={contextValue}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings(): SettingsContextValue {
  const settings = useContext(SettingsContext);
  if (settings === null) {
    throw new Error("useSettings must be used within a SettingsProvider.");
  }
  return settings;
}
