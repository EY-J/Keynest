# Responsive Centered Authentication Design

## Goal

Make KeyNest's master-password screens feel like part of the application rather than content inside a modal-style card. The setup, unlock, and local-data-error screens will use a centered, borderless content column that adapts to the available window width and height.

This design supersedes the authentication-card presentation described in `2026-08-10-compact-default-window-design.md`. The existing 1000-by-700 default and minimum native window size remains unchanged.

## Scope

The shared authentication presentation will apply to:

- First-time master-password creation
- Returning-user unlock
- Local encrypted-data error recovery

The destructive reset confirmation remains a true modal dialog because it temporarily interrupts the current screen and requires an explicit decision.

This work changes layout and styling only. It does not change password validation, encryption, stored data, authentication state, cooldown behavior, reset behavior, or backend commands.

## Shared Layout

`AuthLayout` will continue to own the shared authentication shell, title bar, brand mark, eyebrow, heading, description, and child content. Its inner wrapper will be named and styled as regular page content rather than as a card.

The content column will:

- Use the existing dark KeyNest background and ambient green gradients.
- Remove the card border, card background, rounded container, large inner padding, and card shadow.
- Use `width: min(480px, 100%)` so controls remain comfortably sized without stretching on large windows.
- Keep the full group horizontally centered in the available page area.
- Keep the mark, eyebrow, heading, description, requirement copy, warnings, errors, and button text centered.
- Keep field labels and entered password text left-aligned for scanability and familiar form behavior.
- Keep inputs and primary actions at the full width of the content column.

The setup, unlock, and data-error screens will inherit this shared layout. Screen-specific forms and actions remain responsible only for their existing content and behavior.

## Responsive Behavior

Responsiveness depends on both viewport width and viewport height rather than on a single fixed desktop arrangement.

### Standard and Large Windows

Above 1100 pixels wide and 760 pixels tall, the authentication group will be centered vertically and horizontally whenever its content fits. The column remains capped at 480 pixels so maximizing the window adds breathing room instead of stretching fields and text.

### Default and Short Windows

At the native 1000-by-700 default size, and whenever the viewport is at most 1100 pixels wide or at most 760 pixels tall, the layout will reduce decorative space before reducing usability. Page padding, the brand-mark size, heading size, description margin, and vertical form gaps will tighten. Inputs and actionable controls will retain their existing comfortable heights.

The compact rules will respond to height as well as width, including a short-but-wide window. This prevents the setup form from being clipped merely because the viewport is wider than a conventional width breakpoint.

### Content Overflow

The page will center the authentication group only while it fits in the available height. If validation text, an error, system text scaling, or another content increase makes the group taller than the viewport, the group will begin near the top padding and the page will scroll naturally. No important control may be hidden below an unscrollable centered container.

### Defensive Narrow Layout

Although the native Tauri window cannot be resized below 1000 by 700 pixels, the existing narrow breakpoint remains useful for browser development, display scaling, and future host changes. At 520 pixels wide and below, side padding and heading size will reduce further while the content column, inputs, and actions remain fluid and usable.

## Screen Details

### First-Time Setup

The setup screen keeps both password fields, the 12-character requirement, the no-recovery warning, validation feedback, and the create button. The heading and supporting copy are centered. The warning remains visually distinct but its text is centered with the rest of the supporting content.

### Unlock

The unlock screen uses the same centered structure with one password field, error feedback, the unlock button, and the reset link. Cooldown and focus behavior remain unchanged.

### Local Data Error

The data-error screen uses the same centered structure. Its retry and reset actions remain clearly grouped and preserve their existing behavior.

### Reset Confirmation

`ResetDialog` remains visually and semantically modal, including its backdrop, destructive warning, confirmation input, cancellation path, and focus behavior. Removing the outer authentication card must not alter this dialog.

## Accessibility and Error Handling

- The shared heading association through `aria-labelledby` remains intact.
- Existing alert roles, input labels, focus behavior, disabled states, and password visibility controls remain intact.
- Text alignment changes do not remove semantic labels or reduce control sizes.
- Long descriptions, warnings, and error messages wrap inside the content width.
- Keyboard users can reach every action even when the page must scroll.

## Implementation Boundaries

The component change is limited to replacing the modal-like `auth-card` presentation in `AuthLayout` with a semantic content wrapper. Responsive and alignment rules belong in `src/App.css`, using the existing authentication class family and width/height media-query patterns.

No separate mobile component, duplicated authentication layout, JavaScript viewport measurement, or authentication-state change is needed. CSS handles layout adaptation.

The existing uncommitted native window configuration change is outside this styling change and must be preserved.

## Verification

Verification will include:

- Running the existing frontend authentication tests.
- Running the full existing frontend test command.
- Running the production frontend build.
- Checking setup, unlock, and data-error presentation at exactly 1000 by 700 pixels.
- Checking a larger desktop viewport to confirm the content remains centered and capped.
- Checking a short viewport and a narrow browser viewport to confirm compact spacing and natural scrolling.
- Triggering validation or error content to confirm it wraps without clipping controls.
- Confirming the reset confirmation still renders and behaves as a modal dialog.

Verification will test rendered behavior and build output rather than assert brittle CSS source text.

## Success Criteria

- The setup, unlock, and data-error screens no longer appear inside a modal-style card.
- Authentication content is horizontally centered, with centered display and supporting text but left-aligned field labels and typed values.
- The layout is comfortable and fully usable at the 1000-by-700 native minimum.
- Spacing adapts to both width and height, and oversized content scrolls instead of clipping.
- Large windows retain a centered 480-pixel content column without excessive stretching.
- All authentication, encryption, error, cooldown, focus, and reset behavior remains unchanged.
