# KeyNest Logo Design QA

## Comparison Target

- Source visual truth: `C:\Users\Eyy\Downloads\ChatGPT Image Jul 24, 2026, 11_02_57 PM.png`
- Approved source region: the large green nest-and-keyhole symbol on the light half of the brand sheet.
- Implementation master: `src/assets/keynest-mark.png`
- Intended result: symbol only, transparent background, no wordmark, tile, glow, or presentation panel.

## Evidence

### Focused asset comparison

- Comparison: `artifacts/design-qa/keynest-logo-source-vs-transparent-master.png`
- Source pixels: 1448x1086; the 419x283 symbol crop was normalized to an 820x553 visible region inside a 1024x1024 panel.
- Implementation pixels: 1024x1024 RGBA master rendered at native density on KeyNest's dark surface.
- Density normalization: both sides use the same 1024x1024 panel and matching visible bounds before comparison.
- Evidence: silhouette, strand intersections, keyhole proportions, negative spaces, and green gradient match. The transparent implementation removes the source's white matte without leaving a pale halo.

### Full application views

- Compact unlock: `artifacts/design-qa/keynest-unlock-1000x700.png`
  - State: existing local vault, locked.
  - CSS content viewport: 1000x700.
  - Capture pixels: 1268x885, including the 1014x708 outer window frame at Windows scale factor 1.25.
- Large unlock: `artifacts/design-qa/keynest-unlock-1200x800.png`
  - State: existing local vault, locked.
  - CSS content viewport: 1200x800.
  - Capture pixels: 1518x1010, including the 1214x808 outer window frame at Windows scale factor 1.25.
- Home: `artifacts/design-qa/keynest-home-1000x700.png`
  - State: temporary, inert render of the real `HomePage` component; no local vault data was reset or changed.
  - CSS content viewport: 1000x700.
  - Capture pixels: 1268x885 at Windows scale factor 1.25.
- Navigation interaction: `artifacts/design-qa/keynest-home-navigation-open-1000x700.png`
  - State: real navigation opened from the home page by keyboard.
  - Primary interaction checked: the title-bar menu remains keyboard reachable and opens the sidebar.
- Preview method: native Tauri desktop-window capture. The in-app browser bridge was unavailable in this session, so the app was verified in its actual Windows runtime instead.
- Runtime check: the running Tauri/Vite instance showed no frontend exception during the captured states. A separate duplicate preview launch was rejected because port 1420 was already occupied by the existing development instance; it did not affect the captured application.

## Required Fidelity Surfaces

- Fonts and typography: unchanged from the existing KeyNest interface; logo replacement introduces no text, wrapping, weight, or hierarchy changes.
- Spacing and layout rhythm: auth placement remains centered at its compact and large breakpoints; home and title-bar marks align with adjacent brand text without adding a tile or changing surrounding spacing.
- Colors and visual tokens: the extracted green gradient matches the supplied symbol and remains clear against the dark KeyNest surfaces.
- Image quality and asset fidelity: the real supplied raster artwork is used. Transparency, negative spaces, sharpness, and small-size readability were checked at the 18px title-bar placement and at generated 32px, 128px, and 512px native icon sizes.
- Copy and content: all existing application text is unchanged.
- Responsiveness: the auth mark moves from the compact 50px slot at 1000x700 to the 58px base slot above the width-and-height breakpoint without clipping or shifting the centered layout.
- Accessibility and interaction: the shared image is decorative (`alt=""`, `aria-hidden="true"`, non-draggable); visible KeyNest text remains available; the menu stays keyboard operable. The title-bar image uses `pointer-events: none` so the existing drag/maximize parent remains the pointer target.

## Findings

- P0: None.
- P1: None.
- P2: None.

## Comparison History

1. Initial extraction: P2 pale edge visible on a dark background because partly white antialiased source pixels became too opaque.
2. Fix: increased the opaque-distance threshold in the deterministic extraction from 110 to 245 and regenerated the master.
3. Post-fix evidence: `artifacts/design-qa/keynest-logo-source-vs-transparent-master.png` shows a clean dark-surface edge with the source silhouette and gradient preserved.

## Open Questions

- None.

## Implementation Checklist

- [x] Transparent 1024x1024 master matches the selected source symbol.
- [x] Authentication, home header, and custom title bar use the shared mark.
- [x] Tile backgrounds and `K` placeholders are removed from approved placements.
- [x] Tauri desktop and platform icon outputs derive from the same master.
- [x] Compact, large, small-icon, and navigation-open states were inspected.

## Follow-up Polish

- None required for the approved logo-integration scope.

final result: passed
