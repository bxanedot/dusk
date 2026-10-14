# Dusk Catalog — Design

**Status:** Implemented in Dusk 1.9.0.

## Goal

Keep catalog browsing, search, and support details inside Dusk while preserving Dusk’s existing launcher features and navigation.

## User experience

- Show a Dusk Catalog section in game details and a dedicated Dusk Catalog screen.
- Include an offline snapshot of 3,510 unique game entries dated 14 October 2026.
- Search titles and platform names, filter by availability, and paginate results.
- If a game-title lookup has no direct matches, search the bundled entries before showing an empty state.
- Show title, availability, supported platforms, and listed cheat count when known.
- Keep catalog entries informational. Catalog actions never launch another application.
- Group the main sidebar into Start, Play, and Activity; keep collections and Settings accessible; feature the most recent local game on Home; add section navigation to Settings without removing existing controls.
- Use Dusk’s visual system and preserve its game library, saves, collections, profiles, and game-management features.

## Boundaries

- Bundle catalog data in Dusk and load it locally. Browsing and search work offline.
- Do not fetch catalog pages or images at runtime.
- Do not add an application-launch protocol or launch action to catalog entries.
- Do not persist catalog search state or add database migrations.
- Leave Dusk’s normal direct and Steam game-launch paths unchanged.

## Acceptance criteria

1. The catalog loads its bundled entries without a network connection.
2. Searches match title words in any order; platform filters and pagination work.
3. Game-detail search falls back to the bundled catalog when its direct search has no matches.
4. Catalog entries display informational metadata only and cannot launch another application.
5. The UI and user-facing copy use Dusk Catalog branding.
6. Version metadata is 1.9.0 and release notes describe offline catalog browsing.
