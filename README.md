# Rust Server-Rendered Website

A small server-side rendered HTML page built with **Axum** + **Askama** in Rust.

## Layout

```
┌──────────────────────────────────────────────┐
│                  Header                      │
├─────────┬─────────────────────────┬──────────┤
│         │                         │          │
│ Stories │     Main article        │   TOC    │
│ (left)  │     (middle)            │  (right) │
│         │                         │          │
├─────────┴─────────────────────────┴──────────┤
│                  Footer                      │
└──────────────────────────────────────────────┘
```

## Project structure

```
rust-website/
├── Cargo.toml
├── src/
│   └── main.rs           # Axum server + handlers + data
├── templates/
│   └── index.html        # Askama template (compiled into binary)
└── static/
    └── css/
        ├── base.css      # Layout + structure (no colors)
        ├── theme-light.css  # Default theme
        └── theme-dark.css   # Alternative theme
```

## Theming

CSS is split into two layers:

1. **`base.css`** — defines layout, spacing, structure. References color/font variables but does not set them.
2. **`theme-*.css`** — defines `--color-*` and `--font-*` custom properties.

To create a new theme, copy `theme-light.css`, change the variable values, save it as `theme-yourname.css`, and update the `<link>` in `templates/index.html`.

Variables you can override in a theme:

- `--color-bg`, `--color-text`, `--color-muted`, `--color-accent`, `--color-border`, `--color-hover`
- `--color-header-bg`, `--color-header-text`
- `--color-sidebar-bg`, `--color-article-bg`
- `--color-footer-bg`, `--color-footer-text`
- `--font-body`, `--font-heading`
- `--radius`

## Run

```bash
cargo run --release
```

Then open <http://127.0.0.1:3000>.