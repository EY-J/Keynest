import { useEffect, useRef } from "react";
import { SettingsClientError } from "./types";
import { settingsClient } from "./settingsClient";

const ACTIVITY_THROTTLE_MS = 5_000;
const ACTIVITY_ERROR = "KeyNest could not record recent activity.";

type ActivityReporterProps = {
  onError: (message: string) => void;
};

export default function ActivityReporter({ onError }: ActivityReporterProps) {
  const lastSentAt = useRef<number | null>(null);

  useEffect(() => {
    let isMounted = true;

    const reportActivity = () => {
      const now = Date.now();
      if (
        lastSentAt.current !== null &&
        now - lastSentAt.current < ACTIVITY_THROTTLE_MS
      ) {
        return;
      }

      lastSentAt.current = now;
      void settingsClient.recordActivity().catch((error: unknown) => {
        if (
          error instanceof SettingsClientError &&
          error.code === "unauthorized"
        ) {
          return;
        }
        if (isMounted) {
          onError(ACTIVITY_ERROR);
        }
      });
    };

    const eventNames = [
      "pointerdown",
      "keydown",
      "wheel",
      "touchstart",
      "focus",
    ] as const;
    const passiveOptions = { passive: true };
    for (const eventName of eventNames) {
      window.addEventListener(eventName, reportActivity, passiveOptions);
    }

    return () => {
      isMounted = false;
      for (const eventName of eventNames) {
        window.removeEventListener(eventName, reportActivity);
      }
    };
  }, [onError]);

  return null;
}
