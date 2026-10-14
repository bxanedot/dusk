# Dusk Launcher UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved launcher layout to every Dusk screen, prioritizing the user's own Windows game library and launching.

**Architecture:** Retain the existing state and native command boundaries in App. Add a focused game rail alongside compact global navigation, render the existing game details inside the content area, and unify destination styling through the existing stylesheet. Preserve the current data and feature handlers.

**Tech Stack:** React 19, TypeScript, Tauri 2, Lucide React, CSS; existing Rust/SQLite backend.

**Spec:** `docs/superpowers/specs/2026-10-14-dusk-launcher-ui-design.md`

## Global Constraints

- No Glass UI, glass toggle, or new appearance mode. Keep existing Night/OLED/Slate themes and Violet/Ember/Cyan accents.
- Preserve Dusk's crescent logo, custom window controls, controller navigation, and existing feature behavior.
- Local games and their launch actions are real user data. Never seed example games, playtime, screenshots, or achievements.
- Session time is tracked only when Dusk can observe the launched game process. Guest data remains local; signed-in account sync remains optional.
- Keep all existing screens and settings available. This work changes presentation and navigation, not the Catalog/search services or game-tracking rules.
- Use existing local/user game covers and Dusk assets. Do not import assets or branding from the reference launcher.
- Support the existing minimum desktop size of about 1100×700 and typical 1440×900 windows.
- Preserve version 1.9.0. Keep existing untracked `src-tauri/Cargo.lock` and `src-tauri/gen/` out of commits.

## Review Focus

1. Empty local library: Home, Library, and the game rail show useful Scan/Add actions and no invented content. Inspect in Tasks 1 and 3.
2. Large libraries, long titles, missing covers: rail scrolling and fallback art work; selected game and available actions remain legible. Inspect in Tasks 1 and 2.
3. Selected game removed during refresh or without saves/Catalog matches: fall back to the underlying destination or show existing empty states; retain navigation. Inspect in Task 2.
4. Catalog errors and Web search results: availability is distinguishable from local installation, and recovery actions remain available. Inspect in Task 3.
5. Minimum window size, themes, and controller focus: all destinations stay accessible and focus/selection remain visible. Inspect in Tasks 4 and 5.

## File Map

- Create `src/components/GameRail.tsx`: pure presentation of local game records and selection/collapse callbacks.
- Modify `src/App.tsx`: shell composition, collection controls, game workspace placement, card semantics, destination and Settings markup.
- Modify `src/styles.css`: shell, rail, workspace, cards, destination hierarchy, Settings, responsive and focus states.
- Existing Catalog/trainer/account components retain their service interfaces. Add only presentation hooks if their markup requires them, and record any such files in the task commit.
- No backend or database changes are needed.

## Task 1: Build the application and game rails

**Files:** create `src/components/GameRail.tsx`; modify `src/App.tsx` and `src/styles.css`.

**Interface:** export `GameRail(props: GameRailProps)` with:

```ts
interface GameRailProps {
  games: GameRecord[];
  selectedGameId: string | null;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onSelectGame: (gameId: string) => void;
  onOpenLibrary: () => void;
}
```

The component renders records in the supplied order. It owns no inventory or native commands. Cover paths use the existing Tauri file conversion; missing images fall back to title initials. Each game button carries `data-controller-game-id` for the existing controller Play shortcut.

- [ ] Implement the game rail with an accessible collapse control, local-game buttons, selected state, scroll region, and Library action. Include labels/tooltips in the collapsed layout.
- [ ] Wire it to App's existing `games` and `selectedGameId` state; keep collapse state local to the shell.
- [ ] Convert global navigation to a compact rail with all existing destinations, accessible names, selected states, and Settings at the bottom. Retain the title bar, account access, creator link, and top-bar actions.
- [ ] Move collection selection/create/delete controls into the Library area using existing `collectionFilter`, `createCollection`, and `deleteCollection` behavior.
- [ ] Add responsive styling: expanded game rail at wider widths, compact icon rows when space is constrained, and an explicit collapse control. Keep window controls and global navigation visible at 1100×700.
- [ ] Inspect empty/populated rail states, long titles, missing covers, and keyboard selection using available native data; do not seed the user's database.
- [ ] Commit these source files as `feat: add Dusk game navigation rail`.

## Task 2: Promote game details and expose card actions

**Files:** modify `src/App.tsx` and `src/styles.css`.

**Interface:** retain `GameDetail`'s current props and callbacks (`game`, `activeProfile`, `collections`, `memberships`, `onClose`, `onPlay`, `onRefresh`, `onToast`, `onSearchCatalog`, `onBrowseCatalog`). Continue using `selectedGameId` as the selected workspace state; do not add a second inventory or route model.

- [ ] Render `GameDetail` inside main content when its selected game exists; replace the fixed overlay presentation with a full-height workspace that leaves both rails accessible.
- [ ] Preserve the underlying `view` while a game is selected. Back/close clears selection; global navigation clears selection and opens its destination. If refresh removes the selected record, return to that destination.
- [ ] Group existing detail controls under readable headings while retaining Play, favorites, collections, screenshots, trainer/catalog, table search, profile saves, backup/restore, and safety snapshots.
- [ ] Refactor `GameCard` into an article with separate Details, Play, and Favorite buttons, avoiding nested interactive controls. Keep `data-controller-game-id` on the card and update initial console focus to a real button.
- [ ] Style Play and details controls so they remain discoverable without hover; preserve keyboard/controller focus and disabled launch state.
- [ ] Inspect back/global navigation, no-save-folder and no-match states, and focus placement without launching games or changing save files.
- [ ] Commit as `feat: promote game details to launcher workspace`.

## Task 3: Apply the layout to Home and supporting destinations

**Files:** modify `src/App.tsx` and `src/styles.css`; only add presentation hooks to existing child components if needed.

- [ ] Lead Home with the actual most recently played game and visible Play action. For existing games without play history, provide a Library action and real local game content; for an empty library, foreground Scan/Add.
- [ ] Keep recent games and favorites ahead of secondary stats. Use existing covers with readable text overlays and neutral fallbacks.
- [ ] Rework Library/Favorites headings, filter controls, collections, and grid spacing while retaining current filtering, sorting, and membership operations.
- [ ] Apply consistent page headings, surface hierarchy, spacing, and states to Catalog, Screenshots, Achievements, and Web search. Preserve service calls and source/availability labels.
- [ ] Keep the local search and optional Web search scopes visibly distinct and retain current source selection and download/import behavior.
- [ ] Inspect populated/empty pages, Catalog no-match/error states, and Web search scope labels. Confirm existing actions remain reachable.
- [ ] Commit as `feat: refresh Dusk launcher destinations`.

## Task 4: Rework Settings within the shell

**Files:** modify `src/App.tsx` and `src/styles.css`.

- [ ] Organize all existing controls under Account, Profiles & Saves, Appearance, Detection, Installers, Updates, Local Data, Console Mode, and Discord Rich Presence, retaining About/creator content.
- [ ] Use a stable section navigator with visible active location. Preserve existing section IDs/scroll actions where practical, and retain all handlers.
- [ ] Increase small labels/helper text to a readable scale, align controls and descriptions, and provide clear focus/selected/disabled states.
- [ ] Inspect every section at both supported sizes and each existing theme/accent. Confirm no Glass UI preference or styling was introduced.
- [ ] Commit as `feat: refresh Dusk settings layout`.

## Task 5: Review, build locally, and push source

**Files:** UI files above as required for corrections; no backend changes.

- [ ] Run `npm run build`; expect TypeScript and Vite to exit successfully.
- [ ] Run `npm run tauri -- dev` for native visual inspection at 1100×700 and 1440×900. Plain browser preview cannot validate native actions because it lacks the Tauri bridge.
- [ ] Review all routes, game workspace, modals, navigation, keyboard/controller focus, console mode, and existing theme/accent variants. Record any unavailable hardware/runtime checks honestly.
- [ ] Run the requested design detector once after the UI is finished if its installed command is usable; report tool failure accurately and address actionable findings.
- [ ] Run `npm run tauri -- build` to produce the local Windows installer. Preserve the current version and report its exact output path.
- [ ] Complete one independent whole-change review using the execution method selected by the user; fix material findings and repeat affected checks only.
- [ ] Review the final diff and commit only intended files. Keep pre-existing untracked build files out of commits; stop any preview process started for review.
- [ ] Push completed source commits to `origin/main` under the user's earlier authorization. Do not overwrite the existing release/tag; this plan produces a local installer and updated source.

## Plan Review

Spec coverage: shell/rail → Task 1; game actions/workspace → Task 2; Home and supporting screens → Task 3; all Settings → Task 4; accessibility, native visuals, build, and delivery → Task 5. The user explicitly excluded Glass UI. No automated tests that merely mirror styling are added; meaningful integration checks are the production/native builds and targeted native inspection.
