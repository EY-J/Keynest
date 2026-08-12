import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import AppTitleBar from "./AppTitleBar";

describe("AppTitleBar", () => {
  it("renders the KeyNest mark beside the application name", () => {
    const { container } = render(<AppTitleBar />);

    expect(
      container.querySelector(
        ".titlebar-app-name > img.brand-mark.titlebar-brand-mark",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("KeyNest")).toBeInTheDocument();
  });
});
