import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { settingsClient } from "./settingsClient";
import ActivityReporter from "./ActivityReporter";

vi.mock("./settingsClient", () => ({
  settingsClient: {
    recordActivity: vi.fn(),
  },
}));

describe("ActivityReporter", () => {
  const recordActivity = vi.mocked(settingsClient.recordActivity);

  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    recordActivity.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports the first activity immediately and throttles all supported events for five seconds", async () => {
    render(<ActivityReporter onError={vi.fn()} />);

    for (const eventName of [
      "pointerdown",
      "keydown",
      "wheel",
      "touchstart",
      "focus",
    ]) {
      act(() => window.dispatchEvent(new Event(eventName)));
    }

    expect(recordActivity).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
      window.dispatchEvent(new Event("keydown"));
    });

    expect(recordActivity).toHaveBeenCalledTimes(2);
  });

  it("removes activity listeners when unmounted", () => {
    const { unmount } = render(<ActivityReporter onError={vi.fn()} />);

    unmount();
    act(() => window.dispatchEvent(new Event("pointerdown")));

    expect(recordActivity).not.toHaveBeenCalled();
  });
});
