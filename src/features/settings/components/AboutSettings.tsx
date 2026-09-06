import { getVersion } from "@tauri-apps/api/app";
import { FolderOpen, Info } from "lucide-react";
import { useEffect, useState } from "react";
import { settingsClient } from "../settingsClient";
import SettingsRow from "./SettingsRow";

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
    <div className="settings-row-list settings-section-body">
      <SettingsRow
        icon={Info}
        title="KeyNest"
        description="Local-first secure personal vault."
      >
        <span className="settings-version">
          {version === null ? "Version unavailable" : `Version ${version}`}
        </span>
      </SettingsRow>

      <SettingsRow
        icon={FolderOpen}
        title="Data Location"
        description="Your encrypted local KeyNest data."
      >
        <button
          className="secondary-button compact-button"
          type="button"
          onClick={() => void settingsClient.openDataFolder()}
        >
          Open Folder
        </button>
      </SettingsRow>
    </div>
  );
}
