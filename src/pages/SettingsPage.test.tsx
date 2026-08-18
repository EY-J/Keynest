import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";

vi.mock("../features/settings/components/SecuritySettings", () => ({
  default: () => <div>Security controls</div>,
}));

describe("SettingsPage", () => {
  it("starts on Security and exposes the supported settings categories", () => {
    render(<SettingsPage />);

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
    render(<SettingsPage />);

    await user.click(screen.getByRole("tab", { name: "About" }));

    expect(screen.getByRole("tab", { name: "About" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tabpanel", { name: "About" })).toHaveTextContent(
      "About",
    );
  });
});
