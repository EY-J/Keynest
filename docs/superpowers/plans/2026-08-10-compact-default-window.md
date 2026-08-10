# Compact Default Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make KeyNest open at 1000 by 700 pixels, prevent resizing below that size, and use a more compact presentation near the default viewport while preserving the spacious layout in larger windows.

**Architecture:** Keep native window limits in the existing Tauri JSON configuration and add one width-or-height media query to the existing stylesheet. A static Vitest contract test will import both files as raw text, parse the JSON, and verify the exact configuration and compact CSS declarations without changing React component structure.

**Tech Stack:** Tauri 2 configuration, React 19, CSS media queries, TypeScript 5.8, Vitest 4, Vite raw imports

## Global Constraints

- Default window size is exactly 1000 by 700 pixels.
- Minimum window size is exactly 1000 by 700 pixels.
- The window remains resizable to larger dimensions, centered, and decoration-free.
- Compact mode uses `@media (max-width: 1100px), (max-height: 760px)` so either constrained dimension activates it.
- Compact mode changes presentation only; it does not alter React structure, authentication, storage, encryption, or backend commands.
- Existing input and button dimensions remain unchanged.
- Existing 850-pixel and 520-pixel narrow-screen breakpoints remain in place.

---

## File Structure

- Create `src/app/windowPresentation.test.ts`: regression contract for native window dimensions and compact CSS values.
- Modify `src-tauri/tauri.conf.json`: set the supported native minimum dimensions.
- Modify `src/App.css`: add the compact desktop media query before the existing narrow-screen rules.

### Task 1: Lock the Native Window to the Supported Minimum

**Files:**
- Create: `src/app/windowPresentation.test.ts`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: the first entry in `app.windows` from `src-tauri/tauri.conf.json`.
- Produces: a main-window contract with `width`, `minWidth` equal to 1000; `height`, `minHeight` equal to 700; and `resizable`, `center` true with `decorations` false.

- [ ] **Step 1: Write the failing native-window contract test**

Create `src/app/windowPresentation.test.ts` with the first test and shared imports:

```ts
import { describe, expect, it } from "vitest";
import tauriConfigSource from "../../src-tauri/tauri.conf.json?raw";

type WindowConfig = {
  width: number;
  height: number;
  minWidth: number;
  minHeight: number;
  resizable: boolean;
  center: boolean;
  decorations: boolean;
};

type TauriConfig = {
  app: {
    windows: WindowConfig[];
  };
};

const tauriConfig = JSON.parse(tauriConfigSource) as TauriConfig;

describe("desktop window presentation", () => {
  it("uses 1000 by 700 as both the default and minimum window size", () => {
    expect(tauriConfig.app.windows[0]).toMatchObject({
      width: 1000,
      height: 700,
      minWidth: 1000,
      minHeight: 700,
      resizable: true,
      center: true,
      decorations: false,
    });
  });
});
```

- [ ] **Step 2: Run the focused test and verify the minimum-size assertion fails**

Run:

```powershell
npm.cmd test -- src/app/windowPresentation.test.ts
```

Expected: FAIL because the current `minWidth` is 800 and `minHeight` is 600.

- [ ] **Step 3: Set the native minimum dimensions**

In `src-tauri/tauri.conf.json`, keep the default dimensions and other flags unchanged, but update the minimum values:

```json
"width": 1000,
"height": 700,
"minWidth": 1000,
"minHeight": 700,
"resizable": true,
"center": true,
"decorations": false
```

- [ ] **Step 4: Run the focused test and verify the native-window contract passes**

Run:

```powershell
npm.cmd test -- src/app/windowPresentation.test.ts
```

Expected: PASS with one passing test.

- [ ] **Step 5: Commit the native window constraint**

```powershell
git add src/app/windowPresentation.test.ts src-tauri/tauri.conf.json
git commit -m "feat: enforce default KeyNest window size"
```

### Task 2: Add the Compact Desktop Presentation

**Files:**
- Modify: `src/app/windowPresentation.test.ts`
- Modify: `src/App.css`

**Interfaces:**
- Consumes: existing selectors in `src/App.css` and the raw `appCss` string established in Task 1.
- Produces: an exact compact media-query block before `@media (max-width: 850px)` while leaving interactive-control sizes and larger-window base rules unchanged.

- [ ] **Step 1: Add a failing compact-style contract test**

Add the stylesheet import below the Vitest import:

```ts
import appCss from "../App.css?raw";
```

Then add this test inside the existing `describe` block in `src/app/windowPresentation.test.ts`:

```ts
  it("defines the compact layout for a near-default width or height", () => {
    const compactQuery =
      "@media (max-width: 1100px), (max-height: 760px)";
    const compactStart = appCss.indexOf(compactQuery);
    const narrowStart = appCss.indexOf("@media (max-width: 850px)");
    const mobileStart = appCss.indexOf("@media (max-width: 520px)");

    expect(compactStart).toBeGreaterThan(-1);
    expect(narrowStart).toBeGreaterThan(compactStart);
    expect(mobileStart).toBeGreaterThan(narrowStart);

    const compactCss = appCss.slice(compactStart, narrowStart);

    expect(compactCss).toMatch(/\.topbar\s*\{[^}]*min-height:\s*76px/);
    expect(compactCss).toMatch(
      /\.hero\s*\{[^}]*padding:\s*60px 0 58px/,
    );
    expect(compactCss).toMatch(
      /\.hero h1\s*\{[^}]*font-size:\s*clamp\(2\.8rem, 5vw, 3\.1rem\)/,
    );
    expect(compactCss).toMatch(
      /\.hero-description\s*\{[^}]*margin:\s*20px 0 26px[^}]*font-size:\s*0\.96rem[^}]*line-height:\s*1\.6/,
    );
    expect(compactCss).toMatch(
      /\.features\s*\{[^}]*padding-bottom:\s*64px/,
    );
    expect(compactCss).toMatch(
      /\.section-heading\s*\{[^}]*margin-bottom:\s*22px/,
    );
    expect(compactCss).toMatch(
      /\.section-heading h2\s*\{[^}]*font-size:\s*1\.9rem/,
    );
    expect(compactCss).toMatch(
      /\.feature-card\s*\{[^}]*min-height:\s*270px[^}]*padding:\s*22px/,
    );
    expect(compactCss).toMatch(
      /\.feature-icon\s*\{[^}]*width:\s*46px[^}]*height:\s*46px[^}]*margin-bottom:\s*22px[^}]*font-size:\s*1\.3rem/,
    );
    expect(compactCss).toMatch(
      /\.feature-card h3\s*\{[^}]*font-size:\s*1\.16rem/,
    );
    expect(compactCss).toMatch(
      /\.feature-card p\s*\{[^}]*margin:\s*12px 0 20px[^}]*font-size:\s*0\.93rem[^}]*line-height:\s*1\.55/,
    );
    expect(compactCss).toMatch(
      /\.auth-page\s*\{[^}]*padding:\s*28px 24px/,
    );
    expect(compactCss).toMatch(
      /\.auth-card\s*\{[^}]*padding:\s*34px/,
    );
    expect(compactCss).toMatch(
      /\.auth-mark\s*\{[^}]*width:\s*50px[^}]*height:\s*50px[^}]*margin-bottom:\s*20px/,
    );
    expect(compactCss).toMatch(
      /\.auth-card h1\s*\{[^}]*font-size:\s*2rem/,
    );
    expect(compactCss).toMatch(
      /\.auth-description\s*\{[^}]*margin-bottom:\s*22px[^}]*font-size:\s*0\.95rem[^}]*line-height:\s*1\.55/,
    );
    expect(compactCss).toMatch(/\.auth-form\s*\{[^}]*gap:\s*15px/);

    expect(compactCss).not.toMatch(
      /\.primary-button|\.feature-button|\.auth-field input|\.auth-submit/,
    );
  });
```

- [ ] **Step 2: Run the focused test and verify the compact-style assertion fails**

Run:

```powershell
npm.cmd test -- src/app/windowPresentation.test.ts
```

Expected: FAIL because the compact media query does not exist yet.

- [ ] **Step 3: Add the compact media query**

Insert this block in `src/App.css` immediately before the existing `@media (max-width: 850px)` rule:

```css
@media (max-width: 1100px), (max-height: 760px) {
  .topbar {
    min-height: 76px;
  }

  .hero {
    padding: 60px 0 58px;
  }

  .hero h1 {
    font-size: clamp(2.8rem, 5vw, 3.1rem);
  }

  .hero-description {
    margin: 20px 0 26px;
    font-size: 0.96rem;
    line-height: 1.6;
  }

  .features {
    padding-bottom: 64px;
  }

  .section-heading {
    margin-bottom: 22px;
  }

  .section-heading h2 {
    font-size: 1.9rem;
  }

  .feature-card {
    min-height: 270px;
    padding: 22px;
  }

  .feature-icon {
    width: 46px;
    height: 46px;
    margin-bottom: 22px;
    font-size: 1.3rem;
  }

  .feature-card h3 {
    font-size: 1.16rem;
  }

  .feature-card p {
    margin: 12px 0 20px;
    font-size: 0.93rem;
    line-height: 1.55;
  }

  .auth-page {
    padding: 28px 24px;
  }

  .auth-card {
    padding: 34px;
  }

  .auth-mark {
    width: 50px;
    height: 50px;
    margin-bottom: 20px;
    font-size: 1.35rem;
  }

  .auth-card h1 {
    font-size: 2rem;
  }

  .auth-description {
    margin-bottom: 22px;
    font-size: 0.95rem;
    line-height: 1.55;
  }

  .auth-form {
    gap: 15px;
  }
}
```

- [ ] **Step 4: Run the focused test and verify both contracts pass**

Run:

```powershell
npm.cmd test -- src/app/windowPresentation.test.ts
```

Expected: PASS with two passing tests.

- [ ] **Step 5: Commit the compact presentation**

```powershell
git add src/app/windowPresentation.test.ts src/App.css
git commit -m "style: compact KeyNest default window"
```

### Task 3: Verify the Integrated Desktop Experience

**Files:**
- Verify only: `src-tauri/tauri.conf.json`
- Verify only: `src/App.css`
- Verify only: `src/app/windowPresentation.test.ts`

**Interfaces:**
- Consumes: the native window constraint and compact CSS contract from Tasks 1 and 2.
- Produces: verified frontend, Rust/Tauri, configuration, and viewport behavior with no additional feature surface.

- [ ] **Step 1: Run the complete frontend test suite**

Run:

```powershell
npm.cmd test
```

Expected: all existing and new tests pass.

- [ ] **Step 2: Build the frontend production bundle**

Run:

```powershell
npm.cmd run build
```

Expected: TypeScript and Vite complete successfully.

- [ ] **Step 3: Run Rust formatting, tests, and lint checks**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: formatting is clean, all Rust tests pass, and Clippy reports no warnings.

- [ ] **Step 4: Validate the Tauri configuration through a no-bundle desktop build**

Run:

```powershell
npm.cmd run tauri -- build --debug --no-bundle
```

Expected: the Tauri CLI accepts the configuration and produces a debug desktop executable without bundling installers.

- [ ] **Step 5: Inspect the compact and spacious layouts**

Check whether the existing development server responds:

```powershell
curl.exe -I --max-time 5 http://127.0.0.1:1420
```

If it does not respond, start it with:

```powershell
npm.cmd run dev -- --host 127.0.0.1
```

Use the in-app browser tooling, if operational, to inspect the home and authentication screens at 1000 by 700 pixels and again above 1100 by 760 pixels.

At 1000 by 700, confirm:

- The complete hero heading is visible at about 50 pixels with no overlap.
- Home supporting copy and feature-card content remain readable.
- Authentication titles, descriptions, and long error text fit and wrap inside the card.
- Inputs and buttons retain their existing dimensions.

At the larger viewport, confirm the original base typography and spacing return. If the in-app browser remains unavailable, report the tooling limitation explicitly and rely on the static CSS contract, production build, and the user's native-app visual check without claiming screenshot verification.

- [ ] **Step 6: Confirm the worktree contains only intended changes**

Run:

```powershell
git status --short
git diff --check
```

Expected: no uncommitted implementation changes remain and no whitespace errors are reported.
