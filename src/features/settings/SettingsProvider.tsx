import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
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
  const isMounted = useRef(false);
  const requestGeneration = useRef(0);

  const beginRequest = useCallback(() => {
    requestGeneration.current += 1;
    return requestGeneration.current;
  }, []);

  const isCurrentRequest = useCallback((generation: number) => {
    return isMounted.current && requestGeneration.current === generation;
  }, []);

  const updateSettings = useCallback(
    async (request: () => Promise<SettingsSnapshot>) => {
      const generation = beginRequest();
      const nextSettings = await request();
      if (isCurrentRequest(generation)) {
        setSettings(nextSettings);
      }
    },
    [beginRequest, isCurrentRequest],
  );

  const reload = useCallback(async () => {
    const generation = beginRequest();
    try {
      const nextSettings = await settingsClient.getSettings();
      if (isCurrentRequest(generation)) {
        setSettings(nextSettings);
      }
    } catch {
      if (isCurrentRequest(generation)) {
        setSettings({ ...DEFAULT_SETTINGS, warning: LOAD_FAILURE_WARNING });
      }
    } finally {
      if (isCurrentRequest(generation)) {
        setHasLoaded(true);
      }
    }
  }, [beginRequest, isCurrentRequest]);

  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
      beginRequest();
    };
  }, [beginRequest]);

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
    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", followSystemTheme);
      return () =>
        mediaQuery.removeEventListener("change", followSystemTheme);
    }

    if (typeof mediaQuery.addListener === "function") {
      mediaQuery.addListener(followSystemTheme);
      return () => mediaQuery.removeListener(followSystemTheme);
    }
  }, [settings.theme]);

  const setAutoLockSeconds = useCallback(
    (value: AutoLockSeconds) =>
      updateSettings(() => settingsClient.setAutoLockSeconds(value)),
    [updateSettings],
  );

  const setClipboardClearSeconds = useCallback(
    (value: ClipboardClearSeconds) =>
      updateSettings(() => settingsClient.setClipboardClearSeconds(value)),
    [updateSettings],
  );

  const setTheme = useCallback(
    (value: ThemePreference) => updateSettings(() => settingsClient.setTheme(value)),
    [updateSettings],
  );

  const setLaunchAtStartup = useCallback(
    (enabled: boolean) =>
      updateSettings(() => settingsClient.setLaunchAtStartup(enabled)),
    [updateSettings],
  );

  const resetToDefaults = useCallback(() => {
    beginRequest();
    if (isMounted.current) {
      setSettings(DEFAULT_SETTINGS);
    }
  }, [beginRequest]);

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
