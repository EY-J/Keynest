import { useRef, useState } from "react";
import { Info, Palette, Shield, Settings, UserRound } from "lucide-react";
import ProfileSettings from "../profile/ProfileSettings";
import type { Profile } from "../profile/profileTypes";
import AboutSettings from "./components/AboutSettings";
import AppearanceSettings from "./components/AppearanceSettings";
import GeneralSettings from "./components/GeneralSettings";
import SecuritySettings from "./components/SecuritySettings";

export type SettingsCategory =
  | "profile"
  | "security"
  | "general"
  | "appearance"
  | "about";

const CATEGORIES: Array<{
  id: SettingsCategory;
  label: string;
  description?: string;
  icon: typeof Shield;
}> = [
  {
    id: "profile",
    label: "Profile",
    description: "Manage your local profile.",
    icon: UserRound,
  },
  {
    id: "security",
    label: "Security",
    description: "Manage how KeyNest protects your vault.",
    icon: Shield,
  },
  {
    id: "general",
    label: "General",
    description: "Configure basic KeyNest behavior.",
    icon: Settings,
  },
  {
    id: "appearance",
    label: "Appearance",
    description: "Choose how KeyNest looks.",
    icon: Palette,
  },
  {
    id: "about",
    label: "About",
    description: "KeyNest application information.",
    icon: Info,
  },
];

type SettingsPageProps = {
  profile: Profile;
  onProfileSaved(profile: Profile): void;
  onResetAuthenticated: (
    currentPassword: string,
    confirmation: "RESET KEYNEST",
  ) => Promise<void>;
};

export default function SettingsPage({
  profile,
  onProfileSaved,
  onResetAuthenticated,
}: SettingsPageProps) {
  const [activeCategory, setActiveCategory] =
    useState<SettingsCategory>("profile");
  const tabRefs = useRef<Record<SettingsCategory, HTMLButtonElement | null>>({
    profile: null,
    security: null,
    general: null,
    appearance: null,
    about: null,
  });
  const category = CATEGORIES.find(({ id }) => id === activeCategory)!;

  function moveToCategory(currentCategory: SettingsCategory, direction: number) {
    const currentIndex = CATEGORIES.findIndex(({ id }) => id === currentCategory);
    const nextCategory = CATEGORIES[
      (currentIndex + direction + CATEGORIES.length) % CATEGORIES.length
    ].id;
    setActiveCategory(nextCategory);
    tabRefs.current[nextCategory]?.focus();
  }

  return (
    <main className="settings-page">
      <nav className="settings-category-nav" aria-label="Settings categories">
        <h2 className="settings-nav-title">Settings</h2>
        <div className="settings-tabs" role="tablist">
          {CATEGORIES.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              ref={(element) => {
                tabRefs.current[id] = element;
              }}
              id={`${id}-tab`}
              className={`settings-tab ${activeCategory === id ? "active" : ""}`}
              type="button"
              role="tab"
              tabIndex={activeCategory === id ? 0 : -1}
              aria-selected={activeCategory === id}
              aria-controls={`${id}-panel`}
              onClick={() => setActiveCategory(id)}
              onKeyDown={(event) => {
                if (event.key === "ArrowRight" || event.key === "ArrowDown") {
                  event.preventDefault();
                  moveToCategory(id, 1);
                }
                if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
                  event.preventDefault();
                  moveToCategory(id, -1);
                }
              }}
            >
              <Icon size={20} strokeWidth={1.8} aria-hidden="true" />
              <span>{label}</span>
            </button>
          ))}
        </div>
      </nav>

      <section className="settings-content">
        <section
          id={`${category.id}-panel`}
          className="settings-panel"
          role="tabpanel"
          aria-labelledby={`${category.id}-tab`}
        >
          <header className="settings-section-header">
            <h1>{category.label}</h1>
            {category.description ? <p>{category.description}</p> : null}
          </header>
          {category.id === "profile" ? (
            <ProfileSettings profile={profile} onSaved={onProfileSaved} />
          ) : null}
          {category.id === "security" ? (
            <SecuritySettings onResetAuthenticated={onResetAuthenticated} />
          ) : null}
          {category.id === "general" ? <GeneralSettings /> : null}
          {category.id === "appearance" ? <AppearanceSettings /> : null}
          {category.id === "about" ? <AboutSettings /> : null}
        </section>
      </section>
    </main>
  );
}
