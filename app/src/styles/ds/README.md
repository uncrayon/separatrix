# Separatrix design system (vendored)

These files are copied from the published design system
(https://claude.ai/artifact/X3yCt4mFuodNPdGjjF1KCn), so the app works offline:

- `tokens.json` — the source of truth for colours, type, spacing, radii, sizes.
- `tokens.css` — CSS custom properties and type-style classes generated from it.
- `components.css` — the `ds-*` component classes (base resets, focus ring, every component).
- `fonts/` — Atkinson Hyperlegible Next and Mono (SIL Open Font License 1.1), Latin subset.

Change the design system first, then copy the files here again. App-specific layout lives in
`../app.css`; it may use the tokens but must not restyle `ds-*` classes.
