import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import BrandMark from "../../../shared/components/BrandMark";
import { settingsClient } from "../settingsClient";

export default function AboutSettings() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let isCurrent = true;
    void getVersion()
      .then((value) => {
        if (isCurrent) {
          setVersion(value);
        }
      })
      .catch(() => {
        if (isCurrent) {
          setVersion(null);
        }
      });

    return () => {
      isCurrent = false;
    };
  }, []);

  return (
    <div className="about-settings">
      <BrandMark className="about-mark" />
      <p>{version === null ? "Version unavailable" : `Version ${version}`}</p>
      <p>
        Your encrypted KeyNest data stays on this device. KeyNest has no account
        service and cannot recover a forgotten master password.
      </p>
      <button
        className="secondary-button"
        type="button"
        onClick={() => void settingsClient.openDataFolder()}
      >
        Open KeyNest data folder
      </button>
    </div>
  );
}
