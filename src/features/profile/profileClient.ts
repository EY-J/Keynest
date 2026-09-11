import { invoke } from "@tauri-apps/api/core";
import { publicError } from "../../shared/security/publicErrors";
import {
  MAX_PROFILE_IMAGE_BYTES,
  MAX_PROFILE_IMAGE_EDGE,
  ProfileClientError,
  type AvatarUpdate,
  type Profile,
  type StagedProfileImage,
  type SupportedProfileImageMime,
} from "./profileTypes";

const SUPPORTED_EXTENSIONS = new Map([
  ["png", "image/png"],
  ["jpg", "image/jpeg"],
  ["jpeg", "image/jpeg"],
  ["webp", "image/webp"],
] as const);

type ImageDimensions = { width: number; height: number };
type DecodeImage = (dataUrl: string) => Promise<ImageDimensions>;

export async function prepareProfileImage(
  file: File,
  decodeImage: DecodeImage = decodeBrowserImage,
): Promise<StagedProfileImage> {
  const extension = file.name.toLowerCase().split(".").pop() ?? "";
  const extensionMime = SUPPORTED_EXTENSIONS.get(
    extension as "png" | "jpg" | "jpeg" | "webp",
  );
  if (!extensionMime) {
    throw new ProfileClientError(
      "unsupported-profile-image",
      "Choose a PNG, JPG, or WEBP image.",
    );
  }
  if (file.size === 0) {
    throw new ProfileClientError(
      "invalid-profile-image",
      "KeyNest could not read that image.",
    );
  }
  if (file.size > MAX_PROFILE_IMAGE_BYTES) {
    throw new ProfileClientError(
      "profile-image-too-large",
      "Choose an image no larger than 5 MB.",
    );
  }

  const bytes = new Uint8Array(await file.arrayBuffer());
  const detectedMime = detectImageMime(bytes);
  if (
    detectedMime === null ||
    detectedMime !== extensionMime ||
    (file.type && normalizeMime(file.type) !== detectedMime)
  ) {
    throw new ProfileClientError(
      "unsupported-profile-image",
      "The file must be a valid PNG, JPG, or WEBP image.",
    );
  }

  const dataBase64 = bytesToBase64(bytes);
  const previewUrl = `data:${detectedMime};base64,${dataBase64}`;
  let dimensions: ImageDimensions;
  try {
    dimensions = await decodeImage(previewUrl);
  } catch {
    throw new ProfileClientError(
      "invalid-profile-image",
      "KeyNest could not read that image.",
    );
  }
  if (
    dimensions.width < 1 ||
    dimensions.height < 1 ||
    dimensions.width > MAX_PROFILE_IMAGE_EDGE ||
    dimensions.height > MAX_PROFILE_IMAGE_EDGE ||
    dimensions.width * dimensions.height > 16_000_000
  ) {
    throw new ProfileClientError(
      "invalid-profile-image-dimensions",
      "Choose an image no larger than 4096 pixels on either side.",
    );
  }

  return { mimeType: detectedMime, dataBase64, previewUrl };
}

function normalizeMime(mime: string): string {
  return mime.toLowerCase() === "image/jpg" ? "image/jpeg" : mime.toLowerCase();
}

function detectImageMime(bytes: Uint8Array): SupportedProfileImageMime | null {
  if (
    bytes.length >= 8 &&
    bytes[0] === 0x89 &&
    bytes[1] === 0x50 &&
    bytes[2] === 0x4e &&
    bytes[3] === 0x47 &&
    bytes[4] === 0x0d &&
    bytes[5] === 0x0a &&
    bytes[6] === 0x1a &&
    bytes[7] === 0x0a
  ) {
    return "image/png";
  }
  if (
    bytes.length >= 3 &&
    bytes[0] === 0xff &&
    bytes[1] === 0xd8 &&
    bytes[2] === 0xff
  ) {
    return "image/jpeg";
  }
  if (
    bytes.length >= 12 &&
    String.fromCharCode(...bytes.subarray(0, 4)) === "RIFF" &&
    String.fromCharCode(...bytes.subarray(8, 12)) === "WEBP"
  ) {
    return "image/webp";
  }
  return null;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

async function decodeBrowserImage(dataUrl: string): Promise<ImageDimensions> {
  const image = new Image();
  image.src = dataUrl;
  await image.decode();
  return { width: image.naturalWidth, height: image.naturalHeight };
}

async function invokeProfile<T>(
  command: string,
  argumentsValue?: Record<string, unknown>,
): Promise<T> {
  try {
    return argumentsValue === undefined
      ? await invoke<T>(command)
      : await invoke<T>(command, argumentsValue);
  } catch (error) {
    const { code, message } = publicError(error);
    throw new ProfileClientError(code, message);
  }
}

export const profileClient = {
  getProfile: () => invokeProfile<Profile>("get_profile"),
  saveProfile: (displayName: string, avatarUpdate: AvatarUpdate) =>
    invokeProfile<Profile>("save_profile", { displayName, avatarUpdate }),
};
