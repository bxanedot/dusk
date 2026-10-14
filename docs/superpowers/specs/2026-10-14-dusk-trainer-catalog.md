# Dusk Trainer Catalog — Design

**Status:** Approved for implementation on 2026-10-14; visual direction refined from the user’s follow-up.

## Goal

Show trainer catalog results inside Dusk’s existing game catalog, then let the user open an available trainer.

## User experience

- Add a **Trainer Catalog** section to a selected game’s detail panel.
- Search the supported public catalog using the Dusk game title. Render matching rows in Dusk with the game title, availability, platforms, and cheat count/update date when supplied.
- Keep multiple matches visible so similar titles are not silently treated as the same game.
- For a result marked **Available** with valid catalog IDs, show **Open trainer**. This opens the registered trainer app for the selected game.
- For **Unsupported**, **Unavailable**, or unknown status, show the status and no launch button. A missing cheat count means the count is unknown, not zero.
- Use vendor-neutral product copy throughout Dusk: **Trainer Catalog**, **Search catalog**, **Open trainer**, and “Trainer controls are handled by the trainer app.” Do not show vendor names, logos, hostnames, or protocol strings in the UI, help, notifications, or user-facing release notes.
- Give the panel the polished game-library feel of a modern trainer app: clear game identity, compact result rows, visible availability badges, useful metadata, and a prominent launch action. Use Dusk’s own visual system and keep its library and existing game-management features in place.
- Keep catalog results inside Dusk; do not open a catalog web page. Searching is user initiated and scoped to the selected game. A search failure is shown separately from an empty result.

## Boundaries

- Read only the supported public server-rendered catalog through a fixed HTTPS origin; use bounded response size and a short timeout.
- Keep the service origin and registered app protocol as internal implementation constants. Never display them in Dusk product copy.
- Parse only known catalog result fields and relative result paths. Treat markup/status changes as a catalog error, not as “unsupported”.
- Use numeric title/game IDs supplied by a catalog row to build a validated registered play URI. If the row does not provide a valid ID pair, show the row without a launch action.
- Do not inspect account credentials, call a private local service, run trainer binaries directly, inject code, or manipulate game memory.
- Do not persist catalog results or add database migrations. Search is on demand.
- Leave Dusk’s normal direct and Steam launch paths, saves, collections, profiles, and other game-management features unchanged. Trainer controls remain in the trainer app.

## Acceptance criteria

1. Searching from a Dusk game detail panel shows catalog matches inside Dusk with availability/platform metadata and optional counts/dates.
2. Empty results, network errors, and unsupported page markup have distinct messages and never imply the game is unsupported.
3. Only a row marked Available with valid positive decimal IDs can expose **Open trainer**; the URI is reconstructed safely and handed to the registered trainer app.
4. Search uses only the fixed HTTPS catalog origin and encodes arbitrary game titles as query data.
5. No catalog query occurs until the user activates search, and no catalog result is persisted.
6. Dusk UI, help, notifications, and user-facing release notes contain no provider names, logos, hostnames, or URI schemes.
7. Existing game records and direct/Steam launching remain unchanged.
8. Version metadata advances from 1.8.19 to 1.8.20 and neutral release notes document the in-Dusk catalog and trainer launch.
