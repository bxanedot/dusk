# Dusk Catalog Implementation Notes

## Goal

Provide an offline, searchable catalog in Dusk and keep every catalog interaction in the Dusk interface.

## Architecture

- `src-tauri/data/dusk_catalog.json` contains the catalog snapshot: 3,510 unique entries dated 14 October 2026.
- Rust embeds the JSON with `include_str!`, deserializes it once, and exposes list and title-search commands.
- React handles platform filters, token search, pagination, empty states, and the Dusk Catalog screens.
- Catalog entries are informational; they have no separate-application launch behavior.
- Catalog results are not stored in Dusk’s database.

## Files

- `src-tauri/data/dusk_catalog.json`
- `src-tauri/src/lib.rs`
- `src/types.ts`
- `src/lib/api.ts`
- `src/App.tsx`
- `src/components/TrainerCatalogPanel.tsx`
- `src/components/TrainerCatalogBrowser.tsx`
- `src/styles.css`
- `README.md`
- `CHANGELOG.md`

## Acceptance

- Catalog browsing and search work offline.
- Search matches multiple title words regardless of order.
- Availability filters and pagination update the displayed results.
- Game-detail search can fall back to the bundled catalog.
- No catalog action launches another application.
- Home, Library, Activity, and Settings remain available in the launcher.
