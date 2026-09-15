export const DEFAULT_DISPLAY_NAME = "KeyNest User";
export const MAX_PROFILE_IMAGE_BYTES = 5 * 1024 * 1024;
export const MAX_PROFILE_IMAGE_EDGE = 4096;

export type Profile = {
  displayName: string;
  avatarDataUrl: string | null;
  hasCustomAvatar: boolean;
  isConfigured: boolean;
};

export type AvatarUpdate =
  | { kind: "keep" }
  | { kind: "remove" }
  | {
      kind: "replace";
      mimeType: SupportedProfileImageMime;
      dataBase64: string;
    };

export type SupportedProfileImageMime =
  | "image/png"
  | "image/jpeg"
  | "image/webp";

export type StagedProfileImage = {
  mimeType: SupportedProfileImageMime;
  dataBase64: string;
  previewUrl: string;
};

export const DEFAULT_PROFILE: Profile = {
  displayName: DEFAULT_DISPLAY_NAME,
  avatarDataUrl: null,
  hasCustomAvatar: false,
  isConfigured: false,
};

export class ProfileClientError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "ProfileClientError";
    this.code = code;
  }
}
