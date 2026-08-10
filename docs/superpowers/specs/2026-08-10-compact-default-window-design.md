# Compact Default Window Design

## Goal

Make KeyNest comfortable to use at its default desktop window size. The app will open at 1000 by 700 pixels, remain resizable to larger sizes, and prevent users from resizing it below that supported layout.

## Window Behavior

The main Tauri window will use these values:

- Default width: 1000 pixels
- Default height: 700 pixels
- Minimum width: 1000 pixels
- Minimum height: 700 pixels
- Resizable: enabled

The existing centered, decoration-free window behavior remains unchanged. Users can maximize the app or resize it above the minimum dimensions, but neither dimension can be reduced below the default size.

## Compact Layout

The existing visual style and content hierarchy will remain intact. A compact desktop presentation will apply through the media query `@media (max-width: 1100px), (max-height: 760px)`. This means compact mode remains active while either viewport dimension is near its default minimum.

At that size:

- The top bar minimum height will reduce from 88 to 76 pixels.
- The home hero heading will cap at `3.1rem` (about 50 pixels) instead of using the current large-screen maximum.
- Hero vertical padding will reduce from 100/90 pixels to 60/58 pixels so the primary content fits naturally within a 700-pixel-tall window.
- Hero supporting copy will use `0.96rem` type, `1.6` line height, and tighter margins.
- Section headings will use `1.9rem`; feature cards will use 22-pixel padding and a 270-pixel minimum height, with their icons, headings, and copy reduced proportionally.
- The authentication page will use 28-pixel page padding and 34-pixel card padding. Its mark will reduce to 50 pixels with a 20-pixel bottom margin, and its title will use `2rem`.
- Authentication descriptions and form gaps will tighten without changing the size of fields or actions.
- Inputs, buttons, and other interactive controls will retain comfortable target sizes and readable text.
- Existing narrow-screen breakpoints will remain available as defensive browser behavior, even though the native window cannot be resized below 1000 by 700 pixels.

Above the compact range, the current larger presentation will remain available so maximized and larger windows continue to use the extra space.

## Implementation Boundaries

The window constraint belongs in `src-tauri/tauri.conf.json`. Compact presentation rules belong in `src/App.css`, using media queries based on width and height so a short window receives the compact layout even when it is wider than 1100 pixels.

No React component structure, authentication flow, stored data, encryption behavior, or backend command will change. The work will reuse existing class names and responsive patterns rather than introduce a separate layout system.

## Edge Cases

- A user may resize only one dimension upward; compact rules should still apply while the other dimension remains near its default minimum.
- A maximized or substantially larger window should retain the current spacious styling.
- Text must not clip or overlap at exactly 1000 by 700 pixels.
- Long authentication validation or error messages must continue to wrap inside the card.

## Verification

Verification will cover:

- Confirming the Tauri configuration contains matching 1000-by-700 default and minimum dimensions with resizing enabled.
- Running the existing frontend test suite.
- Running the production frontend build.
- Running the relevant Rust/Tauri checks if configuration validation is included in those commands.
- Inspecting the layout at exactly 1000 by 700 pixels and at a larger desktop size, when the available browser tooling permits visual inspection.

## Success Criteria

KeyNest opens centered at 1000 by 700 pixels, cannot be resized smaller, and remains resizable larger. At the default size, the home and authentication screens feel visibly more compact without sacrificing readability, usable controls, or the larger-window presentation.
