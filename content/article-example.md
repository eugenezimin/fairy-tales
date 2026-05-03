+++
title     = "The Art of the Rust Web Server"
slug      = "art-of-rust-web-server-md"
author    = "Ada Ferrous"
published = "2026-05-02"
+++

Every web framework promises simplicity, speed, and safety. Rust, unusually, can deliver all three at once — but the path there is littered with lifetime annotations and a steep learning curve. ![Misty forest](/static/img/rust.jpg){flow=right}This article walks through building a small but complete server-side-rendered site, explaining the decisions along the way.

By the end you will have a working Axum application, an Askama template, a layered CSS theming system, and enough confidence to extend it in any direction you like.

## Foundations

Before writing a single line of server code it helps to understand what the finished system will look like. The application is a single binary. It reads a TOML config file on startup, loads content from disk, compiles an Askama template, and then serves every request from memory.

### Choosing Axum

Axum sits on top of Tokio and Hyper. It gives you typed extractors, a familiar router API, and Tower middleware compatibility — [all without hiding](/home) the underlying async machinery.

The router is just a value. Routes, middleware layers, and shared state compose in a single expression and are validated at compile time, so misconfigured routes fail to build rather than fail at runtime.

#### State sharing

Axum's `State` extractor clones a cheap `Arc<T>` into each handler. Wrap your config and content bundle in `Arc` once at startup and every handler gets an immutable, thread-safe view of them without locking.

<!-- ad: article-top -->

## Templating with Askama

Askama compiles Jinja-style templates into Rust code at build time. The result is a zero-overhead rendering path: no runtime parsing, no reflection, just a struct with fields that map directly to template variables.

### View models

The template should never see raw domain types. Define a thin view model — a plain struct — that contains exactly what the template needs and nothing else. This decouples your storage schema from your presentation layer and keeps templates readable.

#### Derived data

Some values the template needs are computed, not stored. The table-of-contents list, for example, is derived from the article sections at render time. Produce it in the render function and pass it as a `Vec<TocEntry>` — the template stays dumb.

### Error handling

Askama's `render()` returns a `Result`. Propagate it with `?` and let the handler convert it into a 500 response. Never panic inside a request handler — a single malformed template variable should not crash the process.

<!-- ad: article-mid -->

## CSS architecture

Styling is split into three layers: a structural base, a theme, and a mobile override. Each layer has one job and no knowledge of the others beyond the CSS custom properties they share.

### The theming system

Every color and font is a CSS custom property defined in a theme file. The base layout references those properties but never sets them. Swapping a theme is a one-line change in the config and a server restart.

To create a new theme, copy `theme-light.css`, rename it, and change the variable values. The application will pick it up next time you point `config.toml` at it.

#### Dark mode

The dark theme ships as `theme-dark.css`. It replaces the warm off-whites with deep charcoal surfaces and shifts the accent from brick red to a warm coral. The structural CSS does not change at all.

What you have seen here is a starting point, not a finished product. Add a database for dynamic content, a markdown renderer for richer articles, or an authentication layer for a members-only section. The foundation is solid enough to support all of it.

Rust on the server is not for every project. But for a site that needs predictable latency, confident deployments, and a single artifact to ship — it is hard to beat.

<!-- ad: article-bottom -->