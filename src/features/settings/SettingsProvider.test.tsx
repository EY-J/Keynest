import { StrictMode } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SettingsProvider, { useSettings } from "./SettingsProvider";
import { settingsClient } from "./settingsClient";

vi.mock("./settingsClient", () => ({
  settingsClient: {
    getSettings: vi.fn(),
    setAutoLockSeconds: vi.fn(),
    setClipboardClearSeconds: vi.fn(),
    setTheme: vi.fn(),
    setLaunchAtStartup: vi.fn(),
    recordActivity: vi.fn(),
    openDataFolder: vi.fn(),
  },
}));

type MediaChangeListener = (event: MediaQueryListEvent) => void;

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });

  return { promise, reject, resolve };
}

type MediaQueryListenerApi = "both" | "legacy" | "modern";

function installColorScheme(
  initiallyDark = false,
  listenerApi: MediaQueryListenerApi = "both",
) {
  let isDark = initiallyDark;
  const listeners = new Set<MediaChangeListener>();
  const modern = {
    addEventListener: vi.fn((type: string, listener: MediaChangeListener) => {
      if (type === "change") listeners.add(listener);
    }),
    removeEventListener: vi.fn(
      (type: string, listener: MediaChangeListener) => {
        if (type === "change") listeners.delete(listener);
      },
    ),
  };
  const legacy = {
    addListener: vi.fn((listener: MediaChangeListener) => {
      listeners.add(listener);
    }),
    removeListener: vi.fn((listener: MediaChangeListener) => {
      listeners.delete(listener);
    }),
  };
  const query = {
    get matches() {
      return isDark;
    },
    media: "(prefers-color-scheme: dark)",
    onchange: null,
    ...(listenerApi !== "legacy" ? modern : {}),
    ...(listenerApi !== "modern" ? legacy : {}),
    dispatchEvent: vi.fn(),
  } as unknown as MediaQueryList;

  vi.stubGlobal("matchMedia", vi.fn(() => query));

  return {
    legacy,
    modern,
    query,
    change(toDark: boolean) {
      isDark = toDark;
      for (const listener of listeners) {
        listener({ matches: toDark } as MediaQueryListEvent);
      }
    },
  };
}

function SettingsProbe() {
  const {
    settings,
    resetToDefaults,
    setAutoLockSeconds,
    setClipboardClearSeconds,
    setLaunchAtStartup,
    setTheme,
    reload,
  } = useSettings();

  return (
    <section>
      <p data-testid="auto-lock">{settings.autoLockSeconds}</p>
      <p data-testid="clipboard-clear">{settings.clipboardClearSeconds}</p>
      <p data-testid="theme">{settings.theme}</p>
      <p data-testid="launch-at-startup">{String(settings.launchAtStartup)}</p>
      <p data-testid="warning">{settings.warning ?? ""}</p>
      <button onClick={() => void setAutoLockSeconds(60)}>Set auto lock</button>
      <button onClick={() => void setClipboardClearSeconds(10)}>
        Set clipboard clear
      </button>
      <button onClick={() => void setLaunchAtStartup(true)}>Set startup</button>
      <button onClick={() => void setTheme("light").catch(() => {})}>
        Set light
      </button>
      <button onClick={() => void reload()}>Reload</button>
      <button onClick={resetToDefaults}>Reset</button>
    </section>
  );
}

describe("SettingsProvider", () => {
  const getSettings = vi.mocked(settingsClient.getSettings);
  const setAutoLockSeconds = vi.mocked(settingsClient.setAutoLockSeconds);
  const setClipboardClearSeconds = vi.mocked(
    settingsClient.setClipboardClearSeconds,
  );
  const setLaunchAtStartup = vi.mocked(settingsClient.setLaunchAtStartup);
  const setTheme = vi.mocked(settingsClient.setTheme);

  beforeEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.style.colorScheme = "";
    installColorScheme();
    getSettings.mockResolvedValue({
      autoLockSeconds: 900,
      clipboardClearSeconds: 60,
      theme: "system",
      launchAtStartup: true,
    });
    setAutoLockSeconds.mockResolvedValue({
      autoLockSeconds: 60,
      clipboardClearSeconds: 60,
      theme: "system",
      launchAtStartup: true,
    });
    setClipboardClearSeconds.mockResolvedValue({
      autoLockSeconds: 900,
      clipboardClearSeconds: 10,
      theme: "system",
      launchAtStartup: true,
    });
    setLaunchAtStartup.mockResolvedValue({
      autoLockSeconds: 900,
      clipboardClearSeconds: 60,
      theme: "system",
      launchAtStartup: true,
    });
    setTheme.mockResolvedValue({
      autoLockSeconds: 900,
      clipboardClearSeconds: 60,
      theme: "light",
      launchAtStartup: true,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("waits to render children until initial preferences load", async () => {
    const pending = deferred<Awaited<ReturnType<typeof getSettings>>>();
    getSettings.mockReturnValueOnce(pending.promise);

    render(
      <SettingsProvider>
        <p>Authenticated content</p>
      </SettingsProvider>,
    );

    expect(screen.getByRole("heading", { name: "Preparing your nest…" })).toBeInTheDocument();
    expect(screen.queryByText("Authenticated content")).not.toBeInTheDocument();

    pending.resolve({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
    });

    expect(await screen.findByText("Authenticated content")).toBeInTheDocument();
  });

  it("subscribes, follows, and cleans up with modern MediaQueryList listeners", async () => {
    const media = installColorScheme(true, "modern");
    const { unmount } = render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("theme");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(media.modern.addEventListener).toHaveBeenCalledOnce();

    media.change(false);

    await waitFor(() => {
      expect(document.documentElement).toHaveAttribute("data-theme", "light");
      expect(document.documentElement.style.colorScheme).toBe("light");
    });

    unmount();
    expect(media.modern.removeEventListener).toHaveBeenCalledOnce();
  });

  it("subscribes, follows, and cleans up with legacy MediaQueryList listeners", async () => {
    const media = installColorScheme(true, "legacy");
    const { unmount } = render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("theme");
    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
    expect(media.legacy.addListener).toHaveBeenCalledOnce();

    media.change(false);

    await waitFor(() => {
      expect(document.documentElement).toHaveAttribute("data-theme", "light");
      expect(document.documentElement.style.colorScheme).toBe("light");
    });

    unmount();
    expect(media.legacy.removeListener).toHaveBeenCalledOnce();
  });

  it("uses a deterministic light fallback when matchMedia is unavailable", async () => {
    vi.unstubAllGlobals();

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("theme");
    expect(document.documentElement).toHaveAttribute("data-theme", "light");
    expect(document.documentElement.style.colorScheme).toBe("light");
  });

  it("keeps forced themes unchanged when the Windows color scheme changes", async () => {
    const media = installColorScheme(false);
    getSettings.mockResolvedValueOnce({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "dark",
      launchAtStartup: false,
    });

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("theme");
    media.change(false);

    expect(document.documentElement).toHaveAttribute("data-theme", "dark");
  });

  it("applies a mutation snapshot only after the backend confirms it", async () => {
    const user = userEvent.setup();
    const pending = deferred<Awaited<ReturnType<typeof setTheme>>>();
    setTheme.mockReturnValueOnce(pending.promise);

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    expect(await screen.findByTestId("theme")).toHaveTextContent("system");
    await user.click(screen.getByRole("button", { name: "Set light" }));
    expect(screen.getByTestId("theme")).toHaveTextContent("system");

    pending.resolve({
      autoLockSeconds: 900,
      clipboardClearSeconds: 60,
      theme: "light",
      launchAtStartup: true,
    });

    await waitFor(() => {
      expect(screen.getByTestId("theme")).toHaveTextContent("light");
    });
  });

  it("keeps newer settings when an older reload fails after it", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    expect(await screen.findByTestId("auto-lock")).toHaveTextContent("900");
    const firstReload = deferred<Awaited<ReturnType<typeof getSettings>>>();
    const secondReload = deferred<Awaited<ReturnType<typeof getSettings>>>();
    getSettings.mockReturnValueOnce(firstReload.promise);
    getSettings.mockReturnValueOnce(secondReload.promise);

    await user.click(screen.getByRole("button", { name: "Reload" }));
    await user.click(screen.getByRole("button", { name: "Reload" }));
    await act(async () => {
      secondReload.resolve({
        autoLockSeconds: 60,
        clipboardClearSeconds: 10,
        theme: "light",
        launchAtStartup: false,
      });
      await Promise.resolve();
    });

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("60");
    expect(screen.getByTestId("warning")).toBeEmptyDOMElement();

    await act(async () => {
      firstReload.reject(new Error("older request failed"));
      await Promise.resolve();
    });

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("60");
    expect(screen.getByTestId("clipboard-clear")).toHaveTextContent("10");
    expect(screen.getByTestId("theme")).toHaveTextContent("light");
    expect(screen.getByTestId("warning")).toBeEmptyDOMElement();
  });

  it("ignores a reload that began before StrictMode effect cleanup", async () => {
    const staleReload = deferred<Awaited<ReturnType<typeof getSettings>>>();
    getSettings.mockReturnValueOnce(staleReload.promise);
    getSettings.mockResolvedValueOnce({
      autoLockSeconds: 60,
      clipboardClearSeconds: 10,
      theme: "light",
      launchAtStartup: false,
    });

    render(
      <StrictMode>
        <SettingsProvider>
          <SettingsProbe />
        </SettingsProvider>
      </StrictMode>,
    );

    expect(await screen.findByTestId("auto-lock")).toHaveTextContent("60");

    await act(async () => {
      staleReload.resolve({
        autoLockSeconds: 900,
        clipboardClearSeconds: 60,
        theme: "system",
        launchAtStartup: true,
      });
      await Promise.resolve();
    });

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("60");
    expect(screen.getByTestId("theme")).toHaveTextContent("light");
  });

  it("keeps the newest confirmed mutation when older responses arrive later", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("theme");
    const olderTheme = deferred<Awaited<ReturnType<typeof setTheme>>>();
    const newerAutoLock = deferred<Awaited<ReturnType<typeof setAutoLockSeconds>>>();
    setTheme.mockReturnValueOnce(olderTheme.promise);
    setAutoLockSeconds.mockReturnValueOnce(newerAutoLock.promise);

    await user.click(screen.getByRole("button", { name: "Set light" }));
    await user.click(screen.getByRole("button", { name: "Set auto lock" }));
    await act(async () => {
      newerAutoLock.resolve({
        autoLockSeconds: 60,
        clipboardClearSeconds: 60,
        theme: "system",
        launchAtStartup: true,
      });
      await Promise.resolve();
    });

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("60");
    expect(screen.getByTestId("theme")).toHaveTextContent("system");

    await act(async () => {
      olderTheme.resolve({
        autoLockSeconds: 900,
        clipboardClearSeconds: 60,
        theme: "light",
        launchAtStartup: true,
      });
      await Promise.resolve();
    });

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("60");
    expect(screen.getByTestId("theme")).toHaveTextContent("system");
  });

  it("prevents an outstanding mutation from overwriting reset defaults", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("theme");
    const pendingTheme = deferred<Awaited<ReturnType<typeof setTheme>>>();
    setTheme.mockReturnValueOnce(pendingTheme.promise);

    await user.click(screen.getByRole("button", { name: "Set light" }));
    await user.click(screen.getByRole("button", { name: "Reset" }));
    await act(async () => {
      pendingTheme.resolve({
        autoLockSeconds: 900,
        clipboardClearSeconds: 60,
        theme: "light",
        launchAtStartup: true,
      });
      await Promise.resolve();
    });

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("300");
    expect(screen.getByTestId("clipboard-clear")).toHaveTextContent("30");
    expect(screen.getByTestId("theme")).toHaveTextContent("system");
    expect(screen.getByTestId("launch-at-startup")).toHaveTextContent("false");
  });

  it("keeps the prior state when a backend mutation rejects", async () => {
    const user = userEvent.setup();
    setTheme.mockRejectedValueOnce(new Error("not saved"));

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    expect(await screen.findByTestId("theme")).toHaveTextContent("system");
    await user.click(screen.getByRole("button", { name: "Set light" }));

    await waitFor(() => {
      expect(screen.getByTestId("theme")).toHaveTextContent("system");
    });
  });

  it("restores local secure defaults", async () => {
    const user = userEvent.setup();

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    await screen.findByTestId("auto-lock");
    await user.click(screen.getByRole("button", { name: "Set auto lock" }));
    await user.click(screen.getByRole("button", { name: "Set clipboard clear" }));
    await user.click(screen.getByRole("button", { name: "Set startup" }));
    await user.click(screen.getByRole("button", { name: "Reset" }));

    expect(screen.getByTestId("auto-lock")).toHaveTextContent("300");
    expect(screen.getByTestId("clipboard-clear")).toHaveTextContent("30");
    expect(screen.getByTestId("theme")).toHaveTextContent("system");
    expect(screen.getByTestId("launch-at-startup")).toHaveTextContent("false");
  });

  it("keeps a backend damage warning available to the authenticated shell", async () => {
    getSettings.mockResolvedValueOnce({
      autoLockSeconds: 300,
      clipboardClearSeconds: 30,
      theme: "system",
      launchAtStartup: false,
      warning: "Saved settings were repaired using secure values.",
    });

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    expect(await screen.findByTestId("warning")).toHaveTextContent(
      "Saved settings were repaired using secure values.",
    );
  });

  it("uses secure defaults and the exact warning when preferences cannot load", async () => {
    getSettings.mockRejectedValueOnce(new Error("unavailable"));

    render(
      <SettingsProvider>
        <SettingsProbe />
      </SettingsProvider>,
    );

    expect(await screen.findByTestId("auto-lock")).toHaveTextContent("300");
    expect(screen.getByTestId("clipboard-clear")).toHaveTextContent("30");
    expect(screen.getByTestId("theme")).toHaveTextContent("system");
    expect(screen.getByTestId("launch-at-startup")).toHaveTextContent("false");
    expect(screen.getByTestId("warning")).toHaveTextContent(
      "KeyNest could not load saved preferences. Secure defaults are active.",
    );
  });

});
