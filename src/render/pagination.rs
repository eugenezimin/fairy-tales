//! Article pagination.
//!
//! Splits a flat section list into pages based on plain-text symbol count.
//! A page boundary always falls *between* sections — a section is never split
//! mid-way. The last section that pushes the running total over `limit` ends
//! the current page; the next section starts the next page.
//!
//! If pagination is disabled in config the caller simply never calls this and
//! passes all sections through unchanged.

use crate::domain::{Block, Inline, Section};

// ── Public API ────────────────────────────────────────────────────────────────

/// Split `sections` into pages, each at most `limit` plain-text symbols.
///
/// Returns at least one page (possibly the full section list if it never
/// exceeds the limit). Each inner `Vec` borrows the original `Section` by
/// clone — sections are cheap to clone because they share no heap data with
/// their siblings.
pub fn paginate(sections: &[Section], limit: usize) -> Vec<Vec<Section>> {
    let mut pages: Vec<Vec<Section>> = Vec::new();
    let mut current: Vec<Section> = Vec::new();
    let mut running = 0usize;

    for section in sections {
        let count = section_symbol_count(section);
        current.push(section.clone());
        running += count;

        if running > limit {
            pages.push(std::mem::take(&mut current));
            running = 0;
        }
    }

    if !current.is_empty() {
        pages.push(current);
    }

    if pages.is_empty() {
        pages.push(Vec::new());
    }

    pages
}

/// Given a full section list (all pages), build a mapping from section anchor
/// id → (1-based page number). Used to make TOC links page-aware.
///
/// Sections with no heading contribute nothing to the map.
pub fn build_page_map(pages: &[Vec<Section>]) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for (page_idx, page) in pages.iter().enumerate() {
        let page_num = page_idx + 1;
        for section in page {
            if let Some(h) = &section.heading {
                map.insert(h.id.clone(), page_num);
            }
        }
    }
    map
}

// ── Counting ──────────────────────────────────────────────────────────────────

fn section_symbol_count(s: &Section) -> usize {
    let heading_count = s
        .heading
        .as_ref()
        .map(|h| h.text.chars().count())
        .unwrap_or(0);
    let blocks_count: usize = s.blocks.iter().map(block_symbol_count).sum();
    heading_count + blocks_count
}

fn block_symbol_count(b: &Block) -> usize {
    match b {
        Block::Paragraph(inlines) => inlines.iter().map(inline_symbol_count).sum(),
        Block::CodeBlock { code, .. } => code.chars().count(),
        Block::List { items, .. } => items
            .iter()
            .flat_map(|item| &item.blocks)
            .map(block_symbol_count)
            .sum(),
        Block::Table { headers, rows } => {
            let h: usize = headers
                .iter()
                .flat_map(|cell| cell.iter())
                .map(inline_symbol_count)
                .sum();
            let r: usize = rows
                .iter()
                .flat_map(|row| row.iter())
                .flat_map(|cell| cell.iter())
                .map(inline_symbol_count)
                .sum();
            h + r
        }
        Block::BlockQuote(blocks) => blocks.iter().map(block_symbol_count).sum(),
        // Ads, rules, images — no countable text.
        Block::Ad(_) | Block::Rule => 0,
    }
}

fn inline_symbol_count(i: &Inline) -> usize {
    match i {
        Inline::Text(t) | Inline::Code(t) => t.chars().count(),
        Inline::Strong(ch) | Inline::Em(ch) => ch.iter().map(inline_symbol_count).sum(),
        Inline::Link { children, .. } => children.iter().map(inline_symbol_count).sum(),
        // Image alt text intentionally excluded (structural, not readable content).
        Inline::Image { .. } | Inline::SoftBreak | Inline::HardBreak => 0,
    }
}
