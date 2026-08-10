import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import AuthLayout from "./AuthLayout";

describe("AuthLayout", () => {
  it("renders authentication as labelled page content instead of a dialog card", () => {
    render(
      <AuthLayout
        eyebrow="FIRST-TIME SETUP"
        title="Create your master password"
        description="Protect the encrypted vault on this device."
      >
        <div>Authentication controls</div>
      </AuthLayout>,
    );

    const content = screen
      .getByRole("heading", { name: "Create your master password" })
      .closest("section");

    expect(content).toHaveClass("auth-content");
    expect(content).not.toHaveClass("auth-card");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText("Authentication controls")).toBeInTheDocument();
  });
});
