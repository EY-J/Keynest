import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";

vi.mock("../features/settings/components/SecuritySettings", () => ({
  default: () => <div>Security controls</div>,
}));

vi.mock("../features/settings/components/GeneralSettings", () => ({
  default: () => <div>General controls</div>,
}));

vi.mock("../features/settings/components/AppearanceSettings", () => ({
  default: () => <div>Appearance controls</div>,
}));

vi.mock("../features/settings/components/AboutSettings", () => ({
  default: () => <div>About controls</div>,
}));

describe("SettingsPage", () => {
  it("starts on Security and exposes the supported settings categories", () => {
    render(<SettingsPage onResetAuthenticated={vi.fn().mockResolvedValue(undefined)} />);

    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Security" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(
      screen.getByRole("tabpanel", { name: "Security" }),
    ).toHaveTextContent("Security");
    expect(screen.getByRole("tab", { name: "General" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Appearance" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "About" })).toBeInTheDocument();
  });

  it("selects About and presents its category panel", async () => {
    const user = userEvent.setup();
    render(<SettingsPage onResetAuthenticated={vi.fn().mockResolvedValue(undefined)} />);

    await user.click(screen.getByRole("tab", { name: "About" }));

    expect(screen.getByRole("tab", { name: "About" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tabpanel", { name: "About" })).toHaveTextContent(
      "About",
    );
  });

  it("moves focus and selection through tabs with ArrowLeft and ArrowRight", async () => {
    const user = userEvent.setup();
    render(<SettingsPage onResetAuthenticated={vi.fn().mockResolvedValue(undefined)} />);

    const security = screen.getByRole("tab", { name: "Security" });
    security.focus();
    await user.keyboard("{ArrowRight}");

    const general = screen.getByRole("tab", { name: "General" });
    expect(general).toHaveFocus();
    expect(general).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{ArrowLeft}");
    expect(security).toHaveFocus();
    expect(security).toHaveAttribute("aria-selected", "true");
  });
});
