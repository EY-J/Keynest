import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import HomePage from "./HomePage";

describe("HomePage", () => {
  it("renders the shared KeyNest mark in the home brand block", () => {
    const { container } = render(
      <HomePage
        onOpenPasswordVault={vi.fn()}
        onLockKeynest={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(
      container.querySelector(".brand > img.brand-mark.logo"),
    ).toBeInTheDocument();
    expect(container.querySelector(".brand > div.logo")).not.toBeInTheDocument();
  });
});
