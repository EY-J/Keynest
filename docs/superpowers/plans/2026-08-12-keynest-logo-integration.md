# KeyNest Logo Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Use product-design:image-to-code for the reference-image fidelity and visual-QA phase. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the approved green KeyNest nest-and-keyhole symbol from the supplied artwork as a transparent production asset, display it consistently in the authentication flow, home header, and custom title bar, and regenerate the complete Tauri desktop icon set from the same master.

**Architecture:** Keep one 1024 by 1024 transparent PNG as the visual source of truth under `src/assets`. Render it through a small decorative `BrandMark` React component so every in-app placement shares the same accessibility and image behavior. Generate native icons from that master with the repository's installed Tauri CLI, then verify the actual rendered UI and small native sizes against the supplied source.

**Tech Stack:** PowerShell 5.1, System.Drawing, PNG/RGBA, React 19, TypeScript 5.8, CSS, Vite 7, Vitest 4, Testing Library, Tauri 2

## Global Constraints

- Use only the large green symbol on the light half of `C:\Users\Eyy\Downloads\ChatGPT Image Jul 24, 2026, 11_02_57 PM.png`.
- Do not include the `KeyNest` wordmark, white or black presentation panels, sample app-icon tiles, rounded-square tile, glow, or drop shadow.
- Preserve the supplied symbol's exact silhouette, negative spaces, green gradient, and proportions. This is an extraction, not an AI redraw or logo redesign.
- Produce `src/assets/keynest-mark.png` as a 1024 by 1024 RGBA PNG with transparent corners and balanced clear space.
- If deterministic background removal cannot produce a clean edge without changing the symbol, stop and ask the user before substituting a regenerated or redrawn mark.
- Use the new mark on the normal authentication screens, the home-page brand block, and the custom title bar.
- The title-bar mark must not interfere with dragging or double-click maximize behavior.
- Keep all logo images decorative (`alt=""`, `aria-hidden="true"`) because visible adjacent text already names KeyNest.
- Do not change authentication, encryption, stored data, navigation behavior, window dimensions, minimum window dimensions, or application copy.
- Do not reset or delete local KeyNest data during visual verification.
- Preserve the existing uncommitted `src-tauri/tauri.conf.json` modification and never stage it in any logo commit.
- Exclude `.worktrees/**` from every Vitest command so the retained worktree does not duplicate the test suite.
- Do not add image-processing runtime dependencies to the application.

---

## File Structure

- Create `scripts/branding/extract-keynest-mark.ps1`: deterministic extraction and alpha cleanup for the approved source artwork.
- Create `src/assets/keynest-mark.png`: canonical 1024 by 1024 transparent logo master.
- Create `src/shared/components/BrandMark.tsx`: reusable decorative logo image.
- Create `src/shared/components/BrandMark.test.tsx`: component accessibility and class contract.
- Create `src/shared/components/AppTitleBar.test.tsx`: title-bar placement contract.
- Create `src/pages/HomePage.test.tsx`: home-brand placement contract.
- Modify `src/features/auth/components/AuthLayout.test.tsx`: authentication placement contract.
- Modify `src/features/auth/components/AuthLayout.tsx`: replace the letter placeholder with `BrandMark`.
- Modify `src/shared/components/AppTitleBar.tsx`: add the small mark beside `KeyNest`.
- Modify `src/pages/HomePage.tsx`: replace the letter placeholder with `BrandMark`.
- Modify `src/App.css`: shared image rendering and placement-specific dimensions without tile backgrounds.
- Regenerate `src-tauri/icons/**`: native platform icons derived from the canonical master.
- Create `design-qa.md`: visual comparison evidence and severity-gated findings.

### Task 1: Extract and Validate the Canonical Transparent Mark

**Files:**
- Create: `scripts/branding/extract-keynest-mark.ps1`
- Create: `src/assets/keynest-mark.png`
- Reference only: `C:\Users\Eyy\Downloads\ChatGPT Image Jul 24, 2026, 11_02_57 PM.png`

**Interfaces:**
- Consumes: the approved 1448 by 1086 RGB presentation image.
- Produces: a deterministic 1024 by 1024 `Format32bppArgb` PNG with the source symbol centered on transparency.

- [ ] **Step 1: Add the deterministic extraction script**

Create `scripts/branding/extract-keynest-mark.ps1` with this implementation:

```powershell
param(
  [Parameter(Mandatory = $true)]
  [string]$SourcePath,

  [string]$OutputPath = (
    Join-Path $PSScriptRoot "..\..\src\assets\keynest-mark.png"
  )
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

function Get-ClampedByte([double]$Value) {
  return [byte][Math]::Round([Math]::Max(0, [Math]::Min(255, $Value)))
}

$resolvedSource = (Resolve-Path -LiteralPath $SourcePath).Path
$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

$source = $null
$crop = $null
$cleaned = $null
$output = $null
$graphics = $null

try {
  $source = [Drawing.Bitmap]::FromFile($resolvedSource)

  if ($source.Width -ne 1448 -or $source.Height -ne 1086) {
    throw "Expected the approved 1448x1086 source artwork. Received $($source.Width)x$($source.Height)."
  }

  # Contains only the large symbol on the light presentation, with matte around it.
  $cropRectangle = [Drawing.Rectangle]::new(120, 210, 480, 350)
  $crop = $source.Clone(
    $cropRectangle,
    [Drawing.Imaging.PixelFormat]::Format32bppArgb
  )
  $cleaned = [Drawing.Bitmap]::new(
    $crop.Width,
    $crop.Height,
    [Drawing.Imaging.PixelFormat]::Format32bppArgb
  )

  $minX = $cleaned.Width
  $minY = $cleaned.Height
  $maxX = -1
  $maxY = -1
  $transparentDistance = 18.0
  $opaqueDistance = 110.0

  for ($y = 0; $y -lt $crop.Height; $y++) {
    for ($x = 0; $x -lt $crop.Width; $x++) {
      $pixel = $crop.GetPixel($x, $y)
      $redDistance = 255 - $pixel.R
      $greenDistance = 255 - $pixel.G
      $blueDistance = 255 - $pixel.B
      $distance = [Math]::Sqrt(
        ($redDistance * $redDistance) +
        ($greenDistance * $greenDistance) +
        ($blueDistance * $blueDistance)
      )

      if ($distance -le $transparentDistance) {
        $alpha = 0
      } elseif ($distance -ge $opaqueDistance) {
        $alpha = 255
      } else {
        $alpha = Get-ClampedByte (
          (($distance - $transparentDistance) /
            ($opaqueDistance - $transparentDistance)) * 255
        )
      }

      if ($alpha -eq 0) {
        $cleaned.SetPixel($x, $y, [Drawing.Color]::Transparent)
        continue
      }

      # Remove the white matte from antialiased edge pixels.
      $alphaRatio = $alpha / 255.0
      $red = Get-ClampedByte (
        ($pixel.R - (255 * (1 - $alphaRatio))) / $alphaRatio
      )
      $green = Get-ClampedByte (
        ($pixel.G - (255 * (1 - $alphaRatio))) / $alphaRatio
      )
      $blue = Get-ClampedByte (
        ($pixel.B - (255 * (1 - $alphaRatio))) / $alphaRatio
      )
      $cleaned.SetPixel(
        $x,
        $y,
        [Drawing.Color]::FromArgb($alpha, $red, $green, $blue)
      )

      if ($alpha -ge 16) {
        $minX = [Math]::Min($minX, $x)
        $minY = [Math]::Min($minY, $y)
        $maxX = [Math]::Max($maxX, $x)
        $maxY = [Math]::Max($maxY, $y)
      }
    }
  }

  if ($maxX -lt $minX -or $maxY -lt $minY) {
    throw "No visible logo pixels were extracted."
  }

  $trimRectangle = [Drawing.Rectangle]::new(
    $minX,
    $minY,
    ($maxX - $minX + 1),
    ($maxY - $minY + 1)
  )
  $maximumWidth = 820.0
  $maximumHeight = 680.0
  $scale = [Math]::Min(
    $maximumWidth / $trimRectangle.Width,
    $maximumHeight / $trimRectangle.Height
  )
  $destinationWidth = [int][Math]::Round($trimRectangle.Width * $scale)
  $destinationHeight = [int][Math]::Round($trimRectangle.Height * $scale)
  $destinationRectangle = [Drawing.Rectangle]::new(
    [int][Math]::Round((1024 - $destinationWidth) / 2),
    [int][Math]::Round((1024 - $destinationHeight) / 2),
    $destinationWidth,
    $destinationHeight
  )

  $output = [Drawing.Bitmap]::new(
    1024,
    1024,
    [Drawing.Imaging.PixelFormat]::Format32bppArgb
  )
  $graphics = [Drawing.Graphics]::FromImage($output)
  $graphics.Clear([Drawing.Color]::Transparent)
  $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
  $graphics.CompositingQuality = [Drawing.Drawing2D.CompositingQuality]::HighQuality
  $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::HighQuality
  $graphics.DrawImage(
    $cleaned,
    $destinationRectangle,
    $trimRectangle,
    [Drawing.GraphicsUnit]::Pixel
  )
  $output.Save($resolvedOutput, [Drawing.Imaging.ImageFormat]::Png)
} finally {
  if ($graphics) { $graphics.Dispose() }
  if ($output) { $output.Dispose() }
  if ($cleaned) { $cleaned.Dispose() }
  if ($crop) { $crop.Dispose() }
  if ($source) { $source.Dispose() }
}

Write-Output "Created $resolvedOutput"
```

The fixed crop is intentional: it excludes both wordmarks, both sample tiles, and the dark-side duplicate before alpha processing begins.

- [ ] **Step 2: Run the extractor against the approved source**

Run:

```powershell
& .\scripts\branding\extract-keynest-mark.ps1 `
  -SourcePath "C:\Users\Eyy\Downloads\ChatGPT Image Jul 24, 2026, 11_02_57 PM.png"
```

Expected: `src/assets/keynest-mark.png` is created and the script reports its absolute path.

- [ ] **Step 3: Validate the master image mechanically**

Run:

```powershell
Add-Type -AssemblyName System.Drawing
$image = [Drawing.Bitmap]::FromFile((Resolve-Path "src/assets/keynest-mark.png"))
try {
  if ($image.Width -ne 1024 -or $image.Height -ne 1024) {
    throw "Master must be exactly 1024x1024."
  }
  if (($image.PixelFormat -band [Drawing.Imaging.PixelFormat]::Alpha) -eq 0) {
    throw "Master must contain an alpha channel."
  }
  foreach ($point in @(
    [Drawing.Point]::new(0, 0),
    [Drawing.Point]::new(1023, 0),
    [Drawing.Point]::new(0, 1023),
    [Drawing.Point]::new(1023, 1023)
  )) {
    if ($image.GetPixel($point.X, $point.Y).A -ne 0) {
      throw "Master corners must be transparent."
    }
  }
  if ($image.GetPixel(512, 300).A -eq 0) {
    throw "Expected visible keyhole artwork near the upper center."
  }
  "Master validation passed: $($image.Width)x$($image.Height), $($image.PixelFormat)"
} finally {
  $image.Dispose()
}
```

Expected: the command reports a 1024 by 1024 alpha image with transparent corners and visible central artwork.

- [ ] **Step 4: Inspect the extracted mark before application work**

Open `src/assets/keynest-mark.png` with transparent-background visualization and compare it side by side with the large light-side symbol in the supplied source. Confirm:

- Every nest strand, intersection, keyhole edge, and negative space matches the source.
- The green gradient remains intact.
- No wordmark, panel background, tile, shadow, gray haze, or white fringe remains.
- The mark is optically centered with balanced clear space and remains recognizable when previewed at 32 by 32 pixels.

If any edge is materially changed, adjust only the crop/matte thresholds and rerun the deterministic script. If a clean exact extraction is not achievable, stop and ask the user before using a generated alternative.

- [ ] **Step 5: Commit the extraction tool and master asset**

Run:

```powershell
git diff --check
git add scripts/branding/extract-keynest-mark.ps1 src/assets/keynest-mark.png
git commit -m "assets: add transparent KeyNest brand mark"
```

Confirm `src-tauri/tauri.conf.json` is not staged or included.

### Task 2: Establish the Shared BrandMark Contract

**Files:**
- Create: `src/shared/components/BrandMark.test.tsx`
- Create: `src/shared/components/AppTitleBar.test.tsx`
- Create: `src/pages/HomePage.test.tsx`
- Modify: `src/features/auth/components/AuthLayout.test.tsx`

**Interfaces:**
- `BrandMark` accepts an optional `className` and always emits a decorative, non-draggable image with the shared `brand-mark` class.
- Authentication, home, and title-bar tests assert placement-specific classes without testing raw CSS source.

- [ ] **Step 1: Write the failing BrandMark component test**

Create `src/shared/components/BrandMark.test.tsx`:

```tsx
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import BrandMark from "./BrandMark";

describe("BrandMark", () => {
  it("renders the shared KeyNest mark as a decorative image", () => {
    const { container } = render(<BrandMark className="example-mark" />);
    const mark = container.querySelector("img.brand-mark.example-mark");

    expect(mark).toBeInTheDocument();
    expect(mark).toHaveAttribute("src", expect.stringContaining("keynest-mark.png"));
    expect(mark).toHaveAttribute("alt", "");
    expect(mark).toHaveAttribute("aria-hidden", "true");
    expect(mark).toHaveAttribute("draggable", "false");
  });
});
```

- [ ] **Step 2: Add failing placement tests for all three application surfaces**

In `src/features/auth/components/AuthLayout.test.tsx`, destructure `container` from `render(...)` and add:

```tsx
expect(
  container.querySelector(".auth-content > img.brand-mark.auth-mark"),
).toBeInTheDocument();
expect(screen.queryByText("K")).not.toBeInTheDocument();
```

Create `src/shared/components/AppTitleBar.test.tsx`:

```tsx
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
```

Create `src/pages/HomePage.test.tsx`:

```tsx
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
```

- [ ] **Step 3: Run the focused tests and confirm the intended failures**

Run:

```powershell
npm.cmd test -- `
  src/shared/components/BrandMark.test.tsx `
  src/shared/components/AppTitleBar.test.tsx `
  src/features/auth/components/AuthLayout.test.tsx `
  src/pages/HomePage.test.tsx `
  --exclude ".worktrees/**"
```

Expected: the suite fails because `BrandMark.tsx` does not exist and the three placements still use no image or a `K` placeholder. Failures must not come from unrelated application behavior.

### Task 3: Implement the Shared Mark and Replace Every Placeholder

**Files:**
- Create: `src/shared/components/BrandMark.tsx`
- Modify: `src/features/auth/components/AuthLayout.tsx`
- Modify: `src/shared/components/AppTitleBar.tsx`
- Modify: `src/pages/HomePage.tsx`
- Modify: `src/App.css`
- Test: the four files from Task 2

**Interfaces:**
- Consumes: `src/assets/keynest-mark.png` and an optional placement class.
- Produces: a reusable decorative image plus responsive 58/50/46-pixel auth, 44-pixel home, and 18-pixel title-bar placements.

- [ ] **Step 1: Create the reusable BrandMark component**

Create `src/shared/components/BrandMark.tsx`:

```tsx
import keynestMark from "../../assets/keynest-mark.png";

type BrandMarkProps = {
  className?: string;
};

export default function BrandMark({ className = "" }: BrandMarkProps) {
  const classNames = ["brand-mark", className].filter(Boolean).join(" ");

  return (
    <img
      className={classNames}
      src={keynestMark}
      alt=""
      aria-hidden="true"
      draggable={false}
    />
  );
}
```

- [ ] **Step 2: Replace the authentication placeholder**

In `src/features/auth/components/AuthLayout.tsx`, import:

```tsx
import BrandMark from "../../../shared/components/BrandMark";
```

Replace:

```tsx
<div className="auth-mark" aria-hidden="true">
  K
</div>
```

with:

```tsx
<BrandMark className="auth-mark" />
```

Do not change the shared section, heading relationship, content order, or children.

- [ ] **Step 3: Replace the home-page placeholder**

In `src/pages/HomePage.tsx`, import:

```tsx
import BrandMark from "../shared/components/BrandMark";
```

Replace:

```tsx
<div className="logo">K</div>
```

with:

```tsx
<BrandMark className="logo" />
```

Keep the adjacent `KeyNest` name and tagline unchanged.

- [ ] **Step 4: Add the mark to the draggable title bar**

In `src/shared/components/AppTitleBar.tsx`, import:

```tsx
import BrandMark from "./BrandMark";
```

Change the title block to:

```tsx
<div
  className="titlebar-app-name"
  data-tauri-drag-region
  onDoubleClick={() => void appWindow?.toggleMaximize()}
>
  <BrandMark className="titlebar-brand-mark" />
  <span data-tauri-drag-region>KeyNest</span>
</div>
```

The image remains decorative, and its CSS must use `pointer-events: none` so pointer actions continue to reach the drag-region parent.

- [ ] **Step 5: Replace tile styling with transparent-image styling**

In `src/App.css`, add the shared image rule near the title-bar styles:

```css
.brand-mark {
  display: block;
  flex: 0 0 auto;
  object-fit: contain;
}

.titlebar-brand-mark {
  width: 18px;
  height: 18px;
  pointer-events: none;
}
```

Replace the existing `.logo` rule with:

```css
.logo {
  width: 44px;
  height: 44px;
}
```

Replace the base `.auth-mark` rule with:

```css
.auth-mark {
  width: 58px;
  height: 58px;
  margin: 0 auto 28px;
}
```

Keep the current responsive sizes and margins, but remove obsolete tile-only declarations from the compact rules. The final overrides must be:

```css
@media (max-width: 1100px), (max-height: 760px) {
  .auth-mark {
    width: 50px;
    height: 50px;
    margin-bottom: 20px;
  }
}

@media (max-width: 520px) {
  .auth-mark {
    width: 46px;
    height: 46px;
    margin-bottom: 16px;
  }
}
```

Do not retain `display: grid`, `place-items`, `border-radius`, `background`, placeholder text color/font rules, or glow/shadow on `.logo` or `.auth-mark`. Do not change the surrounding responsive authentication layout.

- [ ] **Step 6: Run the focused logo-placement tests**

Run:

```powershell
npm.cmd test -- `
  src/shared/components/BrandMark.test.tsx `
  src/shared/components/AppTitleBar.test.tsx `
  src/features/auth/components/AuthLayout.test.tsx `
  src/pages/HomePage.test.tsx `
  --exclude ".worktrees/**"
```

Expected: all four files and four tests pass.

- [ ] **Step 7: Run related auth tests and build the frontend**

Run:

```powershell
npm.cmd test -- `
  src/features/auth/components/AuthLayout.test.tsx `
  src/features/auth/components/SetupScreen.test.tsx `
  src/features/auth/components/UnlockScreen.test.tsx `
  --exclude ".worktrees/**"
npm.cmd run build
```

Expected: all seven auth tests pass, then TypeScript and Vite complete successfully. No authentication behavior or layout regression is introduced.

- [ ] **Step 8: Commit the shared component and UI integration**

Run:

```powershell
git diff --check
git add `
  src/shared/components/BrandMark.tsx `
  src/shared/components/BrandMark.test.tsx `
  src/shared/components/AppTitleBar.tsx `
  src/shared/components/AppTitleBar.test.tsx `
  src/features/auth/components/AuthLayout.tsx `
  src/features/auth/components/AuthLayout.test.tsx `
  src/pages/HomePage.tsx `
  src/pages/HomePage.test.tsx `
  src/App.css
git commit -m "feat: apply KeyNest logo across the app"
```

Confirm `src-tauri/tauri.conf.json` remains unstaged.

### Task 4: Regenerate and Validate the Native Icon Set

**Files:**
- Regenerate: `src-tauri/icons/**`
- Source only: `src/assets/keynest-mark.png`

**Interfaces:**
- Consumes: the square transparent master supported by the installed Tauri CLI.
- Produces: PNG, ICO, ICNS, Windows Store, and any default mobile icon outputs emitted by Tauri 2.

- [ ] **Step 1: Generate every platform icon from the canonical master**

Run:

```powershell
npm.cmd run tauri -- icon src/assets/keynest-mark.png
```

Expected: the CLI completes successfully and writes its default output next to `src-tauri/tauri.conf.json`, replacing the existing generic icon assets under `src-tauri/icons`.

- [ ] **Step 2: Validate required files, dimensions, and transparency**

Run:

```powershell
$requiredFiles = @(
  "src-tauri/icons/32x32.png",
  "src-tauri/icons/128x128.png",
  "src-tauri/icons/128x128@2x.png",
  "src-tauri/icons/icon.png",
  "src-tauri/icons/icon.ico",
  "src-tauri/icons/icon.icns"
)
foreach ($path in $requiredFiles) {
  if (-not (Test-Path -LiteralPath $path)) {
    throw "Missing generated icon: $path"
  }
  if ((Get-Item -LiteralPath $path).Length -eq 0) {
    throw "Generated icon is empty: $path"
  }
}

Add-Type -AssemblyName System.Drawing
$expectedPngSizes = @{
  "32x32.png" = 32
  "128x128.png" = 128
  "128x128@2x.png" = 256
  "icon.png" = 512
}
foreach ($entry in $expectedPngSizes.GetEnumerator()) {
  $path = Join-Path "src-tauri/icons" $entry.Key
  $image = [Drawing.Bitmap]::FromFile((Resolve-Path $path))
  try {
    if ($image.Width -ne $entry.Value -or $image.Height -ne $entry.Value) {
      throw "$($entry.Key) has the wrong dimensions."
    }
    if ($image.GetPixel(0, 0).A -ne 0) {
      throw "$($entry.Key) lost transparent padding."
    }
  } finally {
    $image.Dispose()
  }
}
"Native icon validation passed."
```

Expected: all required formats exist, core PNG dimensions match Tauri's expected sizes, and their corners remain transparent.

- [ ] **Step 3: Inspect the smallest generated icons**

Open `src-tauri/icons/32x32.png`, `src-tauri/icons/128x128.png`, and `src-tauri/icons/icon.png` at their native sizes on both light and dark checkerboard backgrounds. Confirm:

- The symbol remains recognizable at 32 pixels.
- The keyhole and nest negative spaces do not close up.
- No white box or edge halo appears.
- Padding and visual centering remain consistent across sizes.

If the 32-pixel result is unreadable, adjust only the canonical asset's clear-space scale, rerun Task 1 validation, and regenerate the entire icon set. Do not hand-edit individual generated sizes.

- [ ] **Step 4: Commit the generated native icons**

Run:

```powershell
git diff --check
git add src-tauri/icons
git commit -m "assets: regenerate Tauri icons from KeyNest mark"
```

Confirm `src-tauri/tauri.conf.json` remains unstaged and unchanged by the icon command.

### Task 5: Perform Visual QA and Final Verification

**Files:**
- Create: `design-qa.md`
- Verify only: `src/assets/keynest-mark.png`
- Verify only: `src/shared/components/BrandMark.tsx`
- Verify only: `src/features/auth/components/AuthLayout.tsx`
- Verify only: `src/shared/components/AppTitleBar.tsx`
- Verify only: `src/pages/HomePage.tsx`
- Verify only: `src/App.css`
- Verify only: `src-tauri/icons/**`
- Preserve: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: the integrated frontend and generated native assets.
- Produces: screenshot-based visual evidence, severity-gated QA notes, green test/build results, and a clean logo-specific working state.

- [ ] **Step 1: Run the complete frontend suite**

Run:

```powershell
npm.cmd test -- --exclude ".worktrees/**"
```

Expected: nine frontend test files and 17 tests pass, including the new component and placement contracts.

- [ ] **Step 2: Build the production frontend and desktop application**

Run:

```powershell
npm.cmd run build
npm.cmd run tauri -- build --debug --no-bundle
```

Expected: TypeScript/Vite and Tauri both complete successfully. The desktop build consumes the regenerated icon files without configuration changes.

- [ ] **Step 3: Inspect the actual UI at compact and large sizes**

Use the in-app browser or the native Tauri window to capture the implemented authentication surface and home surface. Do not clear local data. If the current application state exposes only one surface, use a temporary, uncommitted Vite preview entry that renders the real `AuthLayout`, `HomePage`, and `AppTitleBar` components with inert callbacks, then remove the preview files after capture.

Inspect at:

- 1000 by 700 pixels, the application's compact default/minimum size.
- A larger size above 1100 by 760 pixels.
- A defensive narrow preview at 520 by 700 pixels if browser-hosted previewing is available.

Confirm:

- The auth mark is centered and scales through 58, 50, and 46 pixels without changing the surrounding responsive behavior.
- The home mark aligns with the `KeyNest` name/tagline and has no colored tile behind it.
- The 18-pixel title-bar mark is sharp, vertically aligned, and does not prevent dragging or double-click maximize.
- All instances preserve transparency against KeyNest's dark surfaces.
- No standalone `K` placeholder remains in the three approved placements.
- The visual mark matches the supplied source closely enough that no visible strand, gap, or gradient change is present.

- [ ] **Step 4: Record the visual comparison in `design-qa.md`**

Create `design-qa.md` at the project root with:

```markdown
# KeyNest Logo Design QA

## Reference

- Source: `C:\Users\Eyy\Downloads\ChatGPT Image Jul 24, 2026, 11_02_57 PM.png`
- Approved scope: green nest-and-keyhole symbol only, transparent background, used throughout the app and native icon set.

## Surfaces Checked

- Authentication at 1000x700 and large-window sizing
- Home header brand block
- Custom title bar
- Native 32px, 128px, and 512px PNG icons

## Findings

- P0: None
- P1: None
- P2: None

## Result

Pass. The extracted symbol matches the supplied reference, retains transparent negative space, remains readable at native icon sizes, and is consistently aligned in every approved application surface.
```

Do not record a pass until the screenshots have actually been inspected. If a discrepancy is found, add it with P0/P1/P2 severity, fix it, recapture the affected surface, and repeat until no P0, P1, or P2 findings remain.

- [ ] **Step 5: Commit the QA record**

Run:

```powershell
git add design-qa.md
git commit -m "docs: record KeyNest logo visual QA"
```

Confirm `src-tauri/tauri.conf.json` remains unstaged.

- [ ] **Step 6: Run the final verification gate**

Run:

```powershell
npm.cmd test -- --exclude ".worktrees/**"
npm.cmd run build
npm.cmd run tauri -- build --debug --no-bundle
git diff --check
git status --short
git log --oneline --decorate -6
```

Expected:

- Nine test files and 17 tests pass.
- Frontend and debug desktop builds succeed.
- All four logo implementation commits are present.
- No logo implementation file is left uncommitted.
- The pre-existing `src-tauri/tauri.conf.json` modification is still visible and untouched.
