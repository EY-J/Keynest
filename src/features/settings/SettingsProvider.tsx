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
  const reloadGeneration = useRef(0);
  const mutationGeneration = useRef(0);
  const committedMutationGeneration = useRef(0);

  const beginReload = useCallback(() => {
    reloadGeneration.current += 1;
    return reloadGeneration.current;
  }, []);

  const beginMutation = useCallback(() => {
    mutationGeneration.current += 1;
    return mutationGeneration.current;
  }, []);

  const isCurrentReload = useCallback(
    (
      generation: number,
      mutationsAtStart: number,
      committedMutationsAtStart: number,
    ) => {
      return (
        isMounted.current &&
        reloadGeneration.current === generation &&
        mutationGeneration.current === mutationsAtStart &&
        committedMutationGeneration.current === committedMutationsAtStart
      );
    },
    [],
  );

  const isCurrentMutation = useCallback((generation: number) => {
    return isMounted.current && mutationGeneration.current === generation;
  }, []);

  const updateSettings = useCallback(
    async (request: () => Promise<SettingsSnapshot>) => {
      const generation = beginMutation();
      const nextSettings = await request();
      if (isCurrentMutation(generation)) {
        committedMutationGeneration.current = generation;
        setSettings(nextSettings);
      }
    },
    [beginMutation, isCurrentMutation],
  );

  const reload = useCallback(async () => {
    const generation = beginReload();
    const mutationsAtStart = mutationGeneration.current;
    const committedMutationsAtStart = committedMutationGeneration.current;
    try {
      const nextSettings = await settingsClient.getSettings();
      if (
        isCurrentReload(
          generation,
          mutationsAtStart,
          committedMutationsAtStart,
        )
      ) {
        setSettings(nextSettings);
      }
    } catch {
      if (
        isCurrentReload(
          generation,
          mutationsAtStart,
          committedMutationsAtStart,
        )
      ) {
        setSettings({ ...DEFAULT_SETTINGS, warning: LOAD_FAILURE_WARNING });
      }
    } finally {
      if (
        isCurrentReload(
          generation,
          mutationsAtStart,
          committedMutationsAtStart,
        )
      ) {
        setHasLoaded(true);
      }
    }
  }, [beginReload, isCurrentReload]);

  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
      beginReload();
      beginMutation();
    };
  }, [beginMutation, beginReload]);

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
    beginMutation();
    beginReload();
    if (isMounted.current) {
      setSettings(DEFAULT_SETTINGS);
    }
  }, [beginMutation, beginReload]);

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
