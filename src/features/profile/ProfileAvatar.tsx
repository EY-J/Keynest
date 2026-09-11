import defaultAvatar from "../../assets/keynest-mark.png";

type ProfileAvatarProps = {
  avatarUrl: string | null;
  className: string;
  alt?: string;
};

export default function ProfileAvatar({
  avatarUrl,
  className,
  alt = "",
}: ProfileAvatarProps) {
  return (
    <img
      className={className}
      src={avatarUrl ?? defaultAvatar}
      alt={alt}
      onLoad={(event) => {
        delete event.currentTarget.dataset.fallbackApplied;
      }}
      onError={(event) => {
        if (event.currentTarget.dataset.fallbackApplied === "true") return;
        event.currentTarget.dataset.fallbackApplied = "true";
        event.currentTarget.src = defaultAvatar;
      }}
    />
  );
}
