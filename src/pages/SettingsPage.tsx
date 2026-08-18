import { useRef, useState } from "react";
import AboutSettings from "../features/settings/components/AboutSettings";
import AppearanceSettings from "../features/settings/components/AppearanceSettings";
import GeneralSettings from "../features/settings/components/GeneralSettings";
import SecuritySettings from "../features/settings/components/SecuritySettings";

export type SettingsCategory =
  | "security"
  | "general"
  | "appearance"
  | "about";

const CATEGORIES: Array<{
  id: SettingsCategory;
  label: string;
  description: string;
}> = [
  {
    id: "security",
    label: "Security",
    description: "Review the security preferences that protect your KeyNest.",
  },
  {
    id: "general",
    label: "General",
    description: "Choose how KeyNest behaves on this device.",
  },
  {
    id: "appearance",
    label: "Appearance",
    description: "Personalize how KeyNest looks while you use it.",
  },
  {
    id: "about",
    label: "About",
    description: "KeyNest is a private, local-first space for your important information.",
  },
];

type SettingsPageProps = {
  onResetAuthenticated: (
    currentPassword: string,
    confirmation: "RESET KEYNEST",
  ) => Promise<void>;
};

export default function SettingsPage({
  onResetAuthenticated,
}: SettingsPageProps) {
  const [activeCategory, setActiveCategory] =
    useState<SettingsCategory>("security");
  const tabRefs = useRef<Record<SettingsCategory, HTMLButtonElement | null>>({
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
        <div className="settings-tabs" role="tablist">
          {CATEGORIES.map(({ id, label }) => (
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
              {label}
            </button>
          ))}
        </div>
      </nav>

      <section className="settings-content">
        <p className="eyebrow">KEYNEST PREFERENCES</p>
        <h1>Settings</h1>
        <p className="settings-introduction">
          Set up the parts of KeyNest that make your private space feel right.
        </p>

        <section
          id={`${category.id}-panel`}
          className="settings-panel"
          role="tabpanel"
          aria-labelledby={`${category.id}-tab`}
        >
          <h2>{category.label}</h2>
          <p>{category.description}</p>
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
