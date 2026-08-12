# KeyNest Logo Integration Design

## Goal

Turn the supplied KeyNest artwork into one reusable, transparent symbol-only PNG and use it as the brand mark throughout the application and Windows/Tauri icon set.

The selected mark is the green nest-and-keyhole symbol. The exported asset excludes the `KeyNest` wordmark, split light/dark presentation, square icon tiles, shadows, and all white or black backgrounds.

## Source and Fidelity

The source is `C:\Users\Eyy\Downloads\ChatGPT Image Jul 24, 2026, 11_02_57 PM.png`, a 1448-by-1086 RGB presentation image containing light and dark logo examples.

The implementation will extract the large green symbol from the light-side example rather than generate a new interpretation. Exact extraction is preferred because it preserves the supplied nest geometry, keyhole proportions, and green tonal variation. AI reconstruction or a manual vector redraw is out of scope unless the supplied pixels cannot produce a clean result.

The extraction process will:

- Crop only the upper-left green nest-and-keyhole symbol.
- Remove the near-white presentation background and convert it to alpha.
- Preserve the internal negative spaces between the nest strokes.
- Retain antialiased edges without a white halo.
- Remove presentation shadows, surrounding text, and the example app tile.
- Center the mark on a square transparent canvas with balanced safe padding.
- Export a 1024-by-1024 RGBA master PNG suitable for both in-app scaling and icon generation.

The final master asset will be saved as `src/assets/keynest-mark.png`. The source presentation image will not be copied into the repository.

## Shared Brand Component

A focused `BrandMark` component in `src/shared/components/BrandMark.tsx` will import and render the master PNG. It accepts a `className` so each existing surface controls only its display size and spacing; the image content remains centralized.

All current usages are decorative because nearby visible text or headings already identify KeyNest. The rendered image will therefore use an empty alternative text value and remain hidden from assistive technology to avoid duplicate announcements.

The component will not add a tile, background, border, radius, or generated shape around the image.

## Application Placement

### Authentication Screens

`AuthLayout` will replace the current letter `K` inside `.auth-mark` with `BrandMark`. Setup, unlock, and local-data-error screens inherit the replacement through the shared layout.

The existing responsive dimensions remain the sizing contract: 58 pixels normally, 50 pixels in compact mode, and 46 pixels at the defensive narrow breakpoint. The old green square background, rounded corners, text styling, and box shadow will be removed so only the transparent mark is visible.

### Homepage Brand

`HomePage` will replace the current letter `K` inside `.logo` with `BrandMark`. The existing 44-pixel brand slot remains, but its green square background, rounded corners, letter styling, and box shadow will be removed. The adjacent `KeyNest` name and tagline remain unchanged.

### App Title Bar

`AppTitleBar` will place an 18-pixel `BrandMark` immediately before the visible `KeyNest` app name. The image will ignore pointer events so its existing drag-region parent continues to receive title-bar dragging and double-clicks. The insertion will not alter maximize behavior, the menu button, or window controls.

### Other Surfaces

Text-only uses of the KeyNest name in headings, buttons, footer copy, and descriptions remain text. The sidebar profile avatar is user identity and will not be replaced with the application logo.

## Windows and Tauri Icons

The 1024-pixel transparent master will be the source for regenerating the complete `src-tauri/icons` set through the Tauri icon generator. This includes PNG sizes, Windows `.ico`, macOS `.icns`, store logo, and Windows square logo variants already present in the project.

Generated icons will preserve alpha transparency and balanced safe padding. No white, black, or colored square tile will be introduced. The green symbol must remain recognizable at 16 to 32 pixels; if the finest nest strokes disappear at those sizes, the master crop or padding will be adjusted rather than redrawing the logo.

The current Tauri configuration paths remain unchanged because they already reference the generated icon set.

## Styling and Responsiveness

The transparent image will use `display: block`, fill its assigned brand slot without distortion, and use `object-fit: contain`. Its aspect ratio must remain intact at every breakpoint.

The logo integration does not change the approved 1000-by-700 window behavior or authentication layout. Existing responsive sizing rules remain in control; only obsolete letter-`K` and tile presentation declarations are removed.

The green artwork is expected to remain visible against KeyNest's dark application surfaces and against standard light or dark Windows icon surfaces. No runtime light/dark asset swap is needed.

## Error Handling and Fallbacks

The extraction must be visually inspected on a checkerboard or contrasting backgrounds before it is accepted. It is rejected if it contains opaque corners, white or black fringes, cropped strokes, visible wordmark pixels, shadows from the presentation, or unintended background coverage.

If deterministic extraction cannot produce clean edges, implementation stops before substituting a different design. The fallback is a separate user-approved AI reconstruction or native-transparency workflow; it is not automatic.

## Testing and Verification

Verification will include:

- Confirming `src/assets/keynest-mark.png` is exactly 1024 by 1024, uses RGBA, has transparent corners, and contains a plausible nontransparent subject area.
- Inspecting the master asset against both light and dark checkerboard backgrounds for halos and cropped strokes.
- Testing `BrandMark` as a decorative image with an empty alternative text value.
- Updating component tests to confirm authentication, homepage, and title-bar brand slots render the shared image and no longer render the placeholder `K`.
- Running the complete frontend test suite and production build.
- Regenerating and validating every existing Tauri icon artifact, including alpha-capable PNG and Windows icon outputs.
- Running the no-bundle Tauri build in an isolated target directory so an existing KeyNest process is not interrupted.
- Inspecting the application at 1000 by 700 and a larger viewport, including authentication and homepage states when available.
- Checking the title bar's drag, double-click maximize, navigation, and window-control interactions after inserting the image.

Visual verification must compare the extracted asset with the supplied source and compare the rendered application with the intended placement. Build success alone does not establish visual fidelity.

## Scope Boundaries

This work changes branding assets and their presentation only. It does not alter authentication, encryption, storage, navigation behavior, window sizing, app copy, or user data.

The existing uncommitted `src-tauri/tauri.conf.json` window-size change must be preserved and excluded from logo-specific commits.

## Success Criteria

- A clean 1024-by-1024 transparent PNG contains only the supplied green nest-and-keyhole symbol.
- The authentication screens, homepage brand, and app title bar use the same shared image asset with no background tile.
- Every Windows/Tauri icon is regenerated from the same transparent master and remains recognizable at small sizes.
- No placeholder brand letter `K`, presentation background, wordmark, or example icon tile remains in the replaced brand slots.
- All existing behavior, tests, builds, and the separate window configuration change remain intact.
