# Bundled typography

These six original, unmodified font files are shipped with KeyNest. Font loading
uses only local CSS URLs; no runtime network connection or installed system font
is required. Only upright styles and the weights used by the UI are included.

## IBM Plex Sans

Regular (400), Medium (500), SemiBold (600), Bold (700), in WOFF2 format.

Source: https://github.com/IBM/plex/tree/master/packages/plex-sans/fonts/complete/woff2

License: `public/font-licenses/IBM-Plex-Sans.txt` (SIL Open Font License 1.1).

## Courier Prime

Regular (400) and Bold (700), in the upstream TrueType format.

Source: https://github.com/google/fonts/tree/main/ofl/courierprime

License: `public/font-licenses/Courier-Prime.txt` (SIL Open Font License 1.1).

License paths are relative to the project root. Vite copies these notices into
`dist/font-licenses/` so they are distributed with the application.

## Usage

- `--font-ui`: all normal UI text and form controls.
- `--font-mono`: uppercase section labels, recovery key output, version metadata,
  and semantic code/time elements.
- Generic `sans-serif` and `monospace` fallbacks are resilience fallbacks, not
  additional bundled typefaces.
