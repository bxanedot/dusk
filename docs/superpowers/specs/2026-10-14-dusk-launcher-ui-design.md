# Dusk Launcher UI Redesign

**Status:** awaiting user review
**Date:** 2026-10-14

## Goal

Give Dusk a cohesive, game-first desktop launcher layout, using the spatial hierarchy of established launchers as a reference while retaining Dusk's own identity and functionality. The first job remains managing and launching the user's own Windows game library. The Dusk Catalog, playtime, saves, screenshots, collections, achievements, and search remain supporting destinations.

## Product context and constraints

- Dusk is a local-first Windows desktop launcher built with Tauri, Rust, React, TypeScript, and SQLite.
- Local games and their launch actions are real user data. Never seed example games, playtime, screenshots, or achievements.
- Session time is tracked only when Dusk can observe the launched game process. Guest data remains local; signed-in account sync remains optional.
- Preserve Dusk's crescent logo, dark visual identity, Night/OLED/Slate themes, Violet/Ember/Cyan accents, controller navigation, custom window controls, and existing feature behavior.
- Keep all existing screens and settings available. This work changes presentation and navigation, not the Catalog/search services or game-tracking rules.
- Do not use third-party logos, game artwork, brand names, or copied interface assets in Dusk's UI. The Battle.net desktop launcher is a spatial reference only: [official desktop launcher overview](https://download.battle.net/en-us/desktop).
- The desktop window currently targets a minimum width of about 1100 px and height of 700 px.

## Approaches considered

1. **Persistent Dusk navigation rail plus a separate game rail (selected).** A compact first rail contains Dusk destinations; a neighboring panel lists only games from the user's local Dusk library. The main panel displays the chosen destination or game. This keeps both launcher navigation and game selection visible. Its tradeoff is reduced content width, so rails should be compact and the game panel should be collapsible when space is tight.
2. **One wider sidebar containing destinations and game shortcuts.** This is the least structural change and leaves more width for content, but it continues to mix global sections, collections, account controls, and game selection in one long column.
3. **Game-only rail with supporting pages nested under Home or Library.** This makes game selection dominant but makes Catalog, screenshots, achievements, and Settings harder to find, so it does not fit the agreed supporting-destination hierarchy.

The user approved approach 1: compact Dusk navigation, a distinct rail for the user's own games, and the same launcher shell across Home, Library, Catalog, activity pages, and Settings.

## Information architecture

### Persistent application shell

- **Dusk navigation rail:** Home, Library, Favorites, Dusk Catalog, Screenshots, Achievements, and Settings remain directly reachable. Collections remain visible from the Library area. Use clear selected states, labels or tooltips, and keyboard/controller focus indicators.
- **My Games rail:** list actual local library entries, with cover or icon, title, and a subtle selected/launchable state. Include only games already registered in Dusk; Catalog records and web search results never appear as installed games. Provide an obvious route to the full Library. Keep the rail available across destinations and allow it to collapse to recover content width.
- **Top bar:** retain global library search, account state, and existing actions such as Add game, Scan PC, and Import archive. Keep optional Web search visually distinct from local-library search, with its selected source visible whenever enabled.
- **Main content:** each destination owns a predictable page heading, supporting description, primary action, and content area. Use consistent spacing, card surfaces, loading states, and empty/error messages.

### Home

- Lead with the most recently played game when one exists, using the real installed/local cover when available and a prominent, always-visible Play/Continue action.
- Follow with a compact Recently played / My games section and relevant local playtime summary. Do not let statistics compete with launching.
- For an empty library, explain the next useful actions and make Scan PC and Add game prominent. Do not show an invented featured game.

### Library and game details

- Keep Library as the complete source of truth for locally registered games, with current source filters, sort choices, favorites, and collections.
- Make game-card actions discoverable without hover. Pointer, keyboard, and controller users must be able to see and activate Play and details.
- Selecting a game from the rail or Library opens a full-height game workspace using existing game data and actions. It presents title, source/install status, Play, playtime/session details, favorite and collection controls, screenshots, trainer/catalog information, community-table search, and save backup/restore in clearly grouped sections.
- Preserve current save-file safety behavior, including the pre-restore snapshot. Show that time tracking depends on Dusk observing the process.
- Keep Download/Get game and web discovery actions clearly distinct from launching games already in the user's library.

### Supporting destinations

- **Favorites:** a focused view of locally registered favorites, with the same launch and details actions as Library.
- **Dusk Catalog:** retain current offline catalog and online table-search capabilities, status and source information, and no-match guidance. Make it clear that Catalog availability does not mean the game is installed or locally registered.
- **Screenshots:** retain the local screenshot gallery and its existing discovery/import behavior.
- **Achievements and stats:** retain Dusk-derived milestones based on real local activity. Explain that these are Dusk achievements, not imported achievements from game stores.
- **Collections:** retain creation and membership behavior. Collections organize the user's actual library and do not create game records.
- **Web search:** keep it optional and separately labeled from local search. Preserve source selection and existing backend behavior; web results never imply local ownership or installation.

### Settings

Keep Settings in the persistent Dusk navigation rail. Replace the long, dense presentation with a readable settings workspace and a stable section navigator. Preserve all existing controls and group them under Account, Profiles & Saves, Appearance, Game Detection, Installers, Updates, Local Data, Console Mode, and Discord Rich Presence. Maintain current scope, persistence, account/logout behavior, and update flows. Target readable body copy and clear labels; avoid tiny text and controls that depend only on hover.

## Visual direction

- Use a strong launcher composition: a narrow global rail, a distinct game list, and a broad content canvas with clear title, context, and primary action.
- Retain Dusk's near-black graphite surfaces, crescent logo, and user-selected accent. Apply accents to selection, focus, and primary actions rather than recoloring every surface.
- Use existing user/local game cover art only where available. Fall back to Dusk's own neutral game icon treatment when cover art is missing.
- Improve type hierarchy and contrast for headings, section labels, metadata, and helper copy. Existing settings copy as small as 9–10 px should be brought to a comfortable readable scale.
- Keep layout density appropriate for a Windows desktop and preserve custom title-bar controls. Avoid copying another product's naming, logos, art, proprietary game taxonomy, or exact screen composition.

## Interaction, states, and accessibility

- Every navigation item, game row, card action, and Settings control must work with pointer, keyboard, and controller navigation where Dusk currently supports it.
- Selected location, selected game, focus, disabled state, and availability must be visually distinct. Important actions, especially Play, cannot appear only on hover.
- Provide consistent loading, empty, offline, and error states without blocking access to the rest of the launcher. Keep actionable recovery paths near the message.
- Keep local game search separate from optional Web search. Preserve existing source labels and avoid describing unverified catalog results as locally installed content.
- Respect the user's selected theme and accent, text scaling expectations, and Windows window size. The game rail should collapse when the available content width is constrained.

## Component and data boundaries

- The shell owns global navigation, title-bar integration, search/actions, and the collapsible game rail.
- Destination pages own their existing page-specific controls and data queries; the redesign should avoid duplicating game or account state.
- The selected game workspace reuses current game-detail operations and the existing SQLite/API boundaries. It must not introduce a second game inventory or alter tracking semantics.
- Theme, accent, controller, account, and local-first preferences continue to flow through their current settings/state mechanisms.

## Error handling and non-goals

- A missing cover, empty library, offline Catalog source, unavailable web source, or failed scan should not break shell navigation.
- Native Tauri actions remain unavailable in a plain browser preview; the final visual pass must use the Windows/Tauri runtime for native actions and window behavior.
- No new game-data source, trainer/search provider, statistics calculation, tracking behavior, account flow, or installer behavior is included.
- No Dusk logo redesign or feature removal is part of this work.

## Review and implementation criteria

The redesign is ready when:

1. Home makes the next Play action and the user's real recently played/local games the clearest content.
2. The actual local game rail is separate from global destinations, and every existing destination remains directly reachable.
3. All current Library, Favorites, Catalog, Screenshots, Achievements, Collections, game-detail, search, Settings, theme, and controller behaviors remain available.
4. Catalog and web results cannot be confused with installed/local games; no fabricated game or activity data appears.
5. Play and game-detail actions are visible and operable without hover, including keyboard/controller focus.
6. Settings content is organized, readable, and uses the same Dusk shell.
7. Empty, loading, offline, and error states preserve navigation and show recovery actions.
8. The final UI is reviewed in the native Windows/Tauri runtime at the supported minimum size and a typical desktop size. A source/build check may confirm integration; no new automated tests are required by this design.

## Assessment limits

The source review confirmed the existing route and feature structure. A browser-only preview reached the account gate but could not mount the main UI because `@tauri-apps/api` requires the native Tauri runtime. The current state of Home, Library, and Settings therefore needs visual confirmation during implementation in the native app. Refero's design-search service was unavailable in this session, so no Refero screenshots informed this spec.
