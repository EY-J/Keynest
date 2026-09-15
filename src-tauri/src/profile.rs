use std::{
    fs,
    io::{self, Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use atomic_write_file::AtomicWriteFile;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) const DEFAULT_DISPLAY_NAME: &str = "KeyNest User";
pub(crate) const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024;
const MAX_AVATAR_EDGE: u32 = 4096;
const MAX_AVATAR_PIXELS: u64 = 16_000_000;
const PROFILE_FORMAT_VERSION: u32 = 1;
const PROFILE_DIRECTORY: &str = "profile";
const PROFILE_FILENAME: &str = "profile.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileSnapshot {
    pub display_name: String,
    pub avatar_data_url: Option<String>,
    pub has_custom_avatar: bool,
    pub is_configured: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum AvatarUpdate {
    Keep,
    Remove,
    Replace {
        mime_type: String,
        data_base64: String,
    },
}

#[derive(Clone)]
pub(crate) struct ProfileService {
    store: ProfileStore,
    operation: Arc<Mutex<()>>,
}

impl ProfileService {
    pub(crate) fn new(app_data_dir: PathBuf) -> Self {
        Self {
            store: ProfileStore::new(app_data_dir),
            operation: Arc::new(Mutex::new(())),
        }
    }

    pub(crate) fn snapshot(&self) -> Result<ProfileSnapshot, ProfileError> {
        let _operation = self.lock_operation();
        self.store.snapshot()
    }

    pub(crate) fn save(
        &self,
        display_name: String,
        avatar_update: AvatarUpdate,
    ) -> Result<ProfileSnapshot, ProfileError> {
        let _operation = self.lock_operation();
        let display_name = validate_display_name(display_name)?;
        let (current, _) = self.store.load_metadata()?;

        match avatar_update {
            AvatarUpdate::Keep => {
                let avatar_file = current
                    .avatar_file
                    .filter(|file| self.store.load_avatar(file).is_some());
                self.store
                    .replace_metadata(&StoredProfile::new(display_name, avatar_file))?;
            }
            AvatarUpdate::Remove => {
                self.store
                    .replace_metadata(&StoredProfile::new(display_name, None))?;
                self.store.remove_managed_avatars_except(None)?;
            }
            AvatarUpdate::Replace {
                mime_type,
                data_base64,
            } => {
                if data_base64.len() > ((MAX_AVATAR_BYTES + 2) / 3) * 4 {
                    return Err(ProfileError::ImageTooLarge);
                }
                let bytes = STANDARD
                    .decode(data_base64)
                    .map_err(|_| ProfileError::InvalidImage)?;
                let image_type = validate_avatar(&bytes, &mime_type)?;
                let next_slot = if current
                    .avatar_file
                    .as_deref()
                    .is_some_and(|file| file.starts_with("avatar-a."))
                {
                    'b'
                } else {
                    'a'
                };
                let avatar_file = format!("avatar-{next_slot}.{}", image_type.extension());
                self.store.replace_avatar(&avatar_file, &bytes)?;

                if let Err(error) = self
                    .store
                    .replace_metadata(&StoredProfile::new(display_name, Some(avatar_file.clone())))
                {
                    let _ = self.store.remove_avatar(&avatar_file);
                    return Err(error);
                }
                self.store
                    .remove_managed_avatars_except(Some(&avatar_file))?;
            }
        }

        self.store.snapshot()
    }

    pub(crate) fn reset(&self) -> Result<(), ProfileError> {
        let _operation = self.lock_operation();
        self.store.reset()
    }

    fn lock_operation(&self) -> MutexGuard<'_, ()> {
        self.operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AvatarType {
    Png,
    Jpeg,
    WebP,
}

impl AvatarType {
    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }

    fn mime_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
        }
    }

    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::WebP => ImageFormat::WebP,
        }
    }

    fn from_file_name(file_name: &str) -> Option<Self> {
        match file_name.rsplit_once('.')?.1 {
            "png" => Some(Self::Png),
            "jpg" => Some(Self::Jpeg),
            "webp" => Some(Self::WebP),
            _ => None,
        }
    }
}

fn validate_display_name(display_name: String) -> Result<String, ProfileError> {
    let trimmed = display_name.trim();
    if trimmed.is_empty() {
        return Err(ProfileError::InvalidDisplayName);
    }
    if trimmed.chars().count() > 50 {
        return Err(ProfileError::DisplayNameTooLong);
    }
    Ok(trimmed.to_owned())
}

fn validate_avatar(bytes: &[u8], claimed_mime: &str) -> Result<AvatarType, ProfileError> {
    if bytes.is_empty() {
        return Err(ProfileError::InvalidImage);
    }
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(ProfileError::ImageTooLarge);
    }

    let format = image::guess_format(bytes).map_err(|_| ProfileError::UnsupportedImage)?;
    let image_type = match format {
        ImageFormat::Png if claimed_mime == "image/png" => AvatarType::Png,
        ImageFormat::Jpeg if matches!(claimed_mime, "image/jpeg" | "image/jpg") => AvatarType::Jpeg,
        ImageFormat::WebP if claimed_mime == "image/webp" => AvatarType::WebP,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP => {
            return Err(ProfileError::ImageTypeMismatch)
        }
        _ => return Err(ProfileError::UnsupportedImage),
    };

    let dimensions = ImageReader::with_format(Cursor::new(bytes), image_type.image_format())
        .into_dimensions()
        .map_err(|_| ProfileError::InvalidImage)?;
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > MAX_AVATAR_EDGE
        || dimensions.1 > MAX_AVATAR_EDGE
        || u64::from(dimensions.0) * u64::from(dimensions.1) > MAX_AVATAR_PIXELS
    {
        return Err(ProfileError::InvalidImageDimensions);
    }

    // Header dimensions alone are not enough: fully decode before accepting.
    image::load_from_memory_with_format(bytes, image_type.image_format())
        .map_err(|_| ProfileError::InvalidImage)?;
    Ok(image_type)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProfile {
    format_version: u32,
    display_name: String,
    avatar_file: Option<String>,
}

impl StoredProfile {
    fn new(display_name: String, avatar_file: Option<String>) -> Self {
        Self {
            format_version: PROFILE_FORMAT_VERSION,
            display_name,
            avatar_file,
        }
    }

    fn is_valid(&self) -> bool {
        self.format_version == PROFILE_FORMAT_VERSION
            && validate_display_name(self.display_name.clone()).is_ok()
            && self
                .avatar_file
                .as_deref()
                .is_none_or(is_managed_avatar_name)
    }
}

impl Default for StoredProfile {
    fn default() -> Self {
        Self::new(DEFAULT_DISPLAY_NAME.to_owned(), None)
    }
}

#[derive(Clone)]
struct ProfileStore {
    profile_dir: PathBuf,
}

impl ProfileStore {
    fn new(app_data_dir: PathBuf) -> Self {
        Self {
            profile_dir: app_data_dir.join(PROFILE_DIRECTORY),
        }
    }

    fn load_metadata(&self) -> Result<(StoredProfile, bool), ProfileError> {
        let bytes = match fs::read(self.metadata_path()) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok((StoredProfile::default(), false))
            }
            Err(error) => return Err(ProfileError::Storage(error)),
        };
        let stored: StoredProfile = match serde_json::from_slice::<StoredProfile>(&bytes) {
            Ok(stored) if stored.is_valid() => stored,
            _ => return Ok((StoredProfile::default(), false)),
        };
        Ok((stored, true))
    }

    fn snapshot(&self) -> Result<ProfileSnapshot, ProfileError> {
        let (stored, is_configured) = self.load_metadata()?;
        let avatar = stored
            .avatar_file
            .as_deref()
            .and_then(|file| self.load_avatar(file));
        let avatar_data_url = avatar.map(|(image_type, bytes)| {
            format!(
                "data:{};base64,{}",
                image_type.mime_type(),
                STANDARD.encode(bytes)
            )
        });
        Ok(ProfileSnapshot {
            display_name: stored.display_name,
            has_custom_avatar: avatar_data_url.is_some(),
            avatar_data_url,
            is_configured,
        })
    }

    fn load_avatar(&self, file_name: &str) -> Option<(AvatarType, Vec<u8>)> {
        if !is_managed_avatar_name(file_name) {
            return None;
        }
        let image_type = AvatarType::from_file_name(file_name)?;
        let bytes = fs::read(self.profile_dir.join(file_name)).ok()?;
        validate_avatar(&bytes, image_type.mime_type()).ok()?;
        Some((image_type, bytes))
    }

    fn replace_metadata(&self, stored: &StoredProfile) -> Result<(), ProfileError> {
        fs::create_dir_all(&self.profile_dir).map_err(ProfileError::Storage)?;
        let bytes = serde_json::to_vec_pretty(stored).map_err(ProfileError::Serialization)?;
        atomic_replace(&self.metadata_path(), &bytes)
    }

    fn replace_avatar(&self, file_name: &str, bytes: &[u8]) -> Result<(), ProfileError> {
        if !is_managed_avatar_name(file_name) {
            return Err(ProfileError::InvalidImage);
        }
        fs::create_dir_all(&self.profile_dir).map_err(ProfileError::Storage)?;
        atomic_replace(&self.profile_dir.join(file_name), bytes)
    }

    fn remove_managed_avatars_except(&self, keep: Option<&str>) -> Result<(), ProfileError> {
        for file_name in managed_avatar_names() {
            if keep == Some(file_name) {
                continue;
            }
            self.remove_avatar(file_name)?;
        }
        Ok(())
    }

    fn remove_avatar(&self, file_name: &str) -> Result<(), ProfileError> {
        if !is_managed_avatar_name(file_name) {
            return Ok(());
        }
        match fs::remove_file(self.profile_dir.join(file_name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ProfileError::Storage(error)),
        }
    }

    fn reset(&self) -> Result<(), ProfileError> {
        match fs::remove_file(self.metadata_path()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(ProfileError::Storage(error)),
        }
        self.remove_managed_avatars_except(None)?;
        match fs::remove_dir(&self.profile_dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
            Err(error) => Err(ProfileError::Storage(error)),
        }
    }

    fn metadata_path(&self) -> PathBuf {
        self.profile_dir.join(PROFILE_FILENAME)
    }
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), ProfileError> {
    let mut file = AtomicWriteFile::open(path).map_err(ProfileError::Storage)?;
    file.write_all(bytes).map_err(ProfileError::Storage)?;
    file.sync_all().map_err(ProfileError::Storage)?;
    file.commit().map_err(ProfileError::Storage)
}

fn managed_avatar_names() -> [&'static str; 6] {
    [
        "avatar-a.png",
        "avatar-a.jpg",
        "avatar-a.webp",
        "avatar-b.png",
        "avatar-b.jpg",
        "avatar-b.webp",
    ]
}

fn is_managed_avatar_name(file_name: &str) -> bool {
    managed_avatar_names().contains(&file_name)
}

#[derive(Debug, Error)]
pub(crate) enum ProfileError {
    #[error("display name is required")]
    InvalidDisplayName,
    #[error("display name is too long")]
    DisplayNameTooLong,
    #[error("profile image is too large")]
    ImageTooLarge,
    #[error("profile image type is unsupported")]
    UnsupportedImage,
    #[error("profile image type does not match its contents")]
    ImageTypeMismatch,
    #[error("profile image dimensions are unsupported")]
    InvalidImageDimensions,
    #[error("profile image could not be decoded")]
    InvalidImage,
    #[error("profile metadata could not be serialized")]
    Serialization(serde_json::Error),
    #[error("profile data could not be accessed")]
    Storage(io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_image(format: ImageFormat) -> Vec<u8> {
        let image = image::DynamicImage::new_rgba8(2, 2);
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        bytes.into_inner()
    }

    fn replacement(format: ImageFormat) -> AvatarUpdate {
        let mime_type = match format {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::WebP => "image/webp",
            _ => unreachable!(),
        };
        AvatarUpdate::Replace {
            mime_type: mime_type.to_owned(),
            data_base64: STANDARD.encode(fixture_image(format)),
        }
    }

    #[test]
    fn missing_profile_is_reported_as_not_configured() {
        let temp = tempfile::tempdir().unwrap();
        let service = ProfileService::new(temp.path().to_path_buf());
        assert_eq!(
            service.snapshot().unwrap(),
            ProfileSnapshot {
                display_name: DEFAULT_DISPLAY_NAME.to_owned(),
                avatar_data_url: None,
                has_custom_avatar: false,
                is_configured: false,
            }
        );
    }

    #[test]
    fn display_name_and_each_supported_image_format_persist() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let temp = tempfile::tempdir().unwrap();
            let service = ProfileService::new(temp.path().to_path_buf());
            let saved = service
                .save("  Míng 猫  ".into(), replacement(format))
                .unwrap();
            assert_eq!(saved.display_name, "Míng 猫");
            assert!(saved.has_custom_avatar);

            let reloaded = ProfileService::new(temp.path().to_path_buf())
                .snapshot()
                .unwrap();
            assert_eq!(reloaded, saved);
        }
    }

    #[test]
    fn reset_removes_saved_profile_metadata_and_managed_avatar() {
        let temp = tempfile::tempdir().unwrap();
        let service = ProfileService::new(temp.path().to_path_buf());
        service
            .save("AJ".into(), replacement(ImageFormat::Png))
            .unwrap();

        service.reset().unwrap();

        let snapshot = service.snapshot().unwrap();
        assert!(!snapshot.is_configured);
        assert_eq!(snapshot.avatar_data_url, None);
        assert!(!temp.path().join(PROFILE_DIRECTORY).exists());
    }

    #[test]
    fn replacing_an_image_switches_atomically_managed_slot_and_removes_old_file() {
        let temp = tempfile::tempdir().unwrap();
        let service = ProfileService::new(temp.path().to_path_buf());
        service
            .save("One".into(), replacement(ImageFormat::Png))
            .unwrap();
        let profile_dir = temp.path().join(PROFILE_DIRECTORY);
        assert!(profile_dir.join("avatar-a.png").is_file());

        service
            .save("Two".into(), replacement(ImageFormat::Jpeg))
            .unwrap();
        assert!(!profile_dir.join("avatar-a.png").exists());
        assert!(profile_dir.join("avatar-b.jpg").is_file());
        assert_eq!(service.snapshot().unwrap().display_name, "Two");
    }

    #[test]
    fn removing_an_image_returns_to_the_bundled_default_state() {
        let temp = tempfile::tempdir().unwrap();
        let service = ProfileService::new(temp.path().to_path_buf());
        service
            .save("Name".into(), replacement(ImageFormat::Png))
            .unwrap();

        let removed = service.save("Name".into(), AvatarUpdate::Remove).unwrap();
        assert!(!removed.has_custom_avatar);
        assert_eq!(removed.avatar_data_url, None);
        assert!(managed_avatar_names().iter().all(|file| !temp
            .path()
            .join(PROFILE_DIRECTORY)
            .join(file)
            .exists()));
    }

    #[test]
    fn a_missing_or_undecodable_custom_file_falls_back_without_a_broken_url() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_path_buf());
        store
            .replace_metadata(&StoredProfile::new(
                "Local".into(),
                Some("avatar-a.png".into()),
            ))
            .unwrap();
        assert_eq!(store.snapshot().unwrap().avatar_data_url, None);

        fs::write(store.profile_dir.join("avatar-a.png"), b"not an image").unwrap();
        assert_eq!(store.snapshot().unwrap().avatar_data_url, None);
    }

    #[test]
    fn unsupported_mismatched_oversized_and_invalid_images_are_rejected() {
        assert!(matches!(
            validate_avatar(b"GIF89a", "image/gif"),
            Err(ProfileError::UnsupportedImage)
        ));
        assert!(matches!(
            validate_avatar(&fixture_image(ImageFormat::Png), "image/jpeg"),
            Err(ProfileError::ImageTypeMismatch)
        ));
        assert!(matches!(
            validate_avatar(&vec![0; MAX_AVATAR_BYTES + 1], "image/png"),
            Err(ProfileError::ImageTooLarge)
        ));
        assert!(matches!(
            validate_avatar(b"\x89PNG\r\n\x1a\ninvalid", "image/png"),
            Err(ProfileError::InvalidImage)
        ));
    }

    #[test]
    fn display_name_is_trimmed_bounded_and_unicode_safe() {
        assert_eq!(validate_display_name("  Ana  ".into()).unwrap(), "Ana");
        assert!(matches!(
            validate_display_name("  ".into()),
            Err(ProfileError::InvalidDisplayName)
        ));
        assert!(validate_display_name("猫".repeat(50)).is_ok());
        assert!(matches!(
            validate_display_name("猫".repeat(51)),
            Err(ProfileError::DisplayNameTooLong)
        ));
    }

    #[test]
    fn avatar_update_and_snapshot_use_the_frontend_camel_case_contract() {
        let update: AvatarUpdate = serde_json::from_value(serde_json::json!({
            "kind": "replace",
            "mimeType": "image/png",
            "dataBase64": "fixture"
        }))
        .unwrap();
        assert!(matches!(update, AvatarUpdate::Replace { .. }));

        let value = serde_json::to_value(ProfileSnapshot {
            display_name: "Name".into(),
            avatar_data_url: None,
            has_custom_avatar: false,
            is_configured: true,
        })
        .unwrap();
        assert_eq!(value["displayName"], "Name");
        assert!(value.get("avatarDataUrl").is_some());
        assert!(value.get("hasCustomAvatar").is_some());
        assert_eq!(value["isConfigured"], true);
    }
}
