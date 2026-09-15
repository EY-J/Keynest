import {
  type ChangeEvent,
  type FormEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import { useToast } from "../../components/ui/Toast/ToastProvider";
import { prepareProfileImage, profileClient } from "./profileClient";
import ProfileAvatar from "./ProfileAvatar";
import {
  ProfileClientError,
  type AvatarUpdate,
  type Profile,
} from "./profileTypes";
import "./ProfileSettings.css";

type ProfileSettingsProps = {
  profile: Profile;
  onSaved(profile: Profile): void;
};

export default function ProfileSettings({ profile, onSaved }: ProfileSettingsProps) {
  const [displayName, setDisplayName] = useState(profile.displayName);
  const [avatarUpdate, setAvatarUpdate] = useState<AvatarUpdate>({ kind: "keep" });
  const [previewAvatarUrl, setPreviewAvatarUrl] = useState<string | null>(
    profile.avatarDataUrl,
  );
  const [error, setError] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const { showToast } = useToast();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const selectionGeneration = useRef(0);
  const trimmedName = displayName.trim();
  const isDirty = trimmedName !== profile.displayName || avatarUpdate.kind !== "keep";
  const canRemove = avatarUpdate.kind === "replace" || (
    avatarUpdate.kind === "keep" && profile.hasCustomAvatar
  );

  useEffect(() => {
    setDisplayName(profile.displayName);
    setAvatarUpdate({ kind: "keep" });
    setPreviewAvatarUrl(profile.avatarDataUrl);
    setError("");
  }, [profile.avatarDataUrl, profile.displayName, profile.hasCustomAvatar]);

  useEffect(() => {
    return () => {
      selectionGeneration.current += 1;
    };
  }, []);

  async function choosePhoto(event: ChangeEvent<HTMLInputElement>) {
    const selectedFile = event.currentTarget.files?.[0];
    if (!selectedFile) {
      return;
    }
    event.currentTarget.value = "";

    const generation = ++selectionGeneration.current;
    setError("");
    try {
      const image = await prepareProfileImage(selectedFile);
      if (generation !== selectionGeneration.current) {
        return;
      }
      setAvatarUpdate({
        kind: "replace",
        mimeType: image.mimeType,
        dataBase64: image.dataBase64,
      });
      setPreviewAvatarUrl(image.previewUrl);
    } catch (selectionError) {
      if (generation !== selectionGeneration.current) {
        return;
      }
      setError(
        selectionError instanceof ProfileClientError
          ? selectionError.message
          : "KeyNest could not read that image.",
      );
    }
  }

  function removePhoto() {
    selectionGeneration.current += 1;
    setAvatarUpdate({ kind: "remove" });
    setPreviewAvatarUrl(null);
    setError("");
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!isDirty || isSaving) {
      return;
    }
    if (!trimmedName) {
      setError("");
      showToast({ type: "error", message: "Enter a display name." });
      return;
    }
    if ([...trimmedName].length > 50) {
      setError("Display name must be 50 characters or fewer.");
      return;
    }

    setError("");
    setIsSaving(true);
    try {
      const savedProfile = await profileClient.saveProfile(trimmedName, avatarUpdate);
      setDisplayName(savedProfile.displayName);
      setAvatarUpdate({ kind: "keep" });
      setPreviewAvatarUrl(savedProfile.avatarDataUrl);
      showToast({
        type: "success",
        message: "Profile saved",
        detail: "Your profile was updated successfully.",
      });
      onSaved(savedProfile);
    } catch (saveError) {
      setError(
        saveError instanceof ProfileClientError
          ? saveError.message
          : "KeyNest could not save the local profile.",
      );
    } finally {
      setIsSaving(false);
    }
  }

  return (
      <form className="profile-settings settings-section-body" onSubmit={(event) => void save(event)}>
        <section className="profile-settings-panel">
          <div className="profile-settings-photo">
            <div className="profile-settings-photo-identity">
              <ProfileAvatar
                avatarUrl={previewAvatarUrl}
                className="profile-settings-avatar"
                alt="Profile preview"
              />
              <h2>Profile photo</h2>
            </div>
            <div className="profile-settings-photo-actions">
              {canRemove ? (
                <button
                  className="secondary-button compact-button profile-settings-remove-button"
                  type="button"
                  disabled={isSaving}
                  onClick={removePhoto}
                >
                  Remove photo
                </button>
              ) : null}
              <button
                className="secondary-button compact-button profile-settings-change-photo"
                type="button"
                disabled={isSaving}
                onClick={() => fileInputRef.current?.click()}
              >
                Change photo
              </button>
            </div>
            <input
              ref={fileInputRef}
              className="sr-only"
              type="file"
              accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp"
              tabIndex={-1}
              onChange={(event) => void choosePhoto(event)}
            />
          </div>

          <div className="profile-settings-name">
            <label htmlFor="profile-display-name">Display name</label>
            <div className="profile-settings-name-row">
              <input
                id="profile-display-name"
                value={displayName}
                type="text"
                autoComplete="name"
                disabled={isSaving}
                onChange={(event) => {
                  setDisplayName(event.currentTarget.value);
                  setError("");
                }}
              />
              <button
                className="primary-button compact-button profile-settings-save"
                type="submit"
                disabled={!isDirty || isSaving}
              >
                {isSaving ? "Saving..." : "Save changes"}
              </button>
            </div>
          </div>

          {error ? <p className="profile-settings-message error" role="alert">{error}</p> : null}
        </section>
      </form>
  );
}
