import {
  FileText,
  LockKeyhole,
  Settings,
  Star,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import BrandMark from "../../components/ui/BrandMark";

type HomeDestination =
  | "vault"
  | "notes"
  | "favorites"
  | "recently-deleted"
  | "settings";

type HomePageProps = {
  onNavigate: (destination: HomeDestination) => void;
};

type AvailableDestination = {
  destination: HomeDestination;
  icon: LucideIcon;
  label: string;
};

const availableDestinations: AvailableDestination[] = [
  { destination: "vault", icon: LockKeyhole, label: "Vault" },
  { destination: "notes", icon: FileText, label: "Notes" },
  { destination: "favorites", icon: Star, label: "Favorites" },
  {
    destination: "recently-deleted",
    icon: Trash2,
    label: "Recently Deleted",
  },
  { destination: "settings", icon: Settings, label: "Settings" },
];

export default function HomePage({ onNavigate }: HomePageProps) {
  return (
    <main className="home-content home-welcome">
      <div className="home-welcome__inner">
        <header className="home-welcome__brand">
          <BrandMark className="home-welcome__logo" />
          <h1>KeyNest</h1>
          <p className="home-welcome__tagline">
            Your personal space on Windows.
          </p>
          <p className="home-welcome__introduction">
            Keep useful information and everyday tools together in one
            local-first application.
          </p>
        </header>

        <section
          className="home-welcome__coming-soon"
          aria-labelledby="home-coming-soon-title"
        >
          <p className="home-welcome__eyebrow">HOME EXPERIENCE</p>
          <h2 id="home-coming-soon-title">Coming soon</h2>
          <p className="home-welcome__supporting-copy">
            We&apos;re designing a more personal way to start, continue, and
            access what matters in KeyNest.
          </p>

          <div className="home-welcome__actions">
            <button
              className="primary-button home-welcome__action"
              type="button"
              onClick={() => onNavigate("vault")}
            >
              Open Vault
            </button>
            <button
              className="secondary-button home-welcome__action"
              type="button"
              onClick={() => onNavigate("notes")}
            >
              Open Notes
            </button>
          </div>
        </section>

        <nav
          className="home-welcome__available"
          aria-label="Available KeyNest destinations"
        >
          <p className="home-welcome__eyebrow">AVAILABLE NOW</p>
          <div className="home-welcome__destination-list">
            {availableDestinations.map(
              ({ destination, icon: DestinationIcon, label }) => (
                <button
                  className="home-welcome__destination"
                  type="button"
                  key={destination}
                  onClick={() => onNavigate(destination)}
                >
                  <DestinationIcon
                    size={15}
                    strokeWidth={1.8}
                    aria-hidden="true"
                  />
                  <span>{label}</span>
                </button>
              ),
            )}
          </div>
        </nav>
      </div>
    </main>
  );
}
