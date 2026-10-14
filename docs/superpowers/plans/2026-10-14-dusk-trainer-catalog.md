# Dusk Trainer Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show trainer catalog results inside the Dusk library and open an available trainer from a Dusk game detail panel.

**Architecture:** Add a bounded Rust fetcher and parser for the supported public server-rendered catalog. Return result metadata to React on explicit search. Parse positive title/game IDs from each official result row; only a result with `Available` status and a validated ID pair gets an **Open trainer** action. Opening uses the registered trainer application protocol. Catalog results stay in memory and no database schema changes are needed.

**Tech Stack:** Rust, Tauri 2, reqwest, existing regex helpers, React, TypeScript.

**Spec:** `docs/superpowers/specs/2026-10-14-dusk-trainer-catalog.md`

## Global Constraints

- Fetch only from a fixed, approved HTTPS catalog origin. Reject blank or overlong queries, set a short timeout, disable redirects, cap the response body at 2 MB, and cap parsed rows at 25.
- Keep the provider origin and registered app protocol as internal constants. Do not expose names, logos, hostnames, or URI schemes in Dusk UI, notifications, help, or user-facing release notes.
- Parse only public result rows and known fields. Reject invalid IDs, non-relative result paths, and unsupported page markup safely. Normalize unknown row status to `unknown`; it must never produce an Open trainer action. Missing optional counts/dates stay `None`.
- Only rows with exact `Available` status and a valid positive decimal ID pair can launch. Do not infer availability from a blank count.
- Reconstruct the play URI from validated IDs and dispatch it via the registered Windows URL handler without waiting.
- Search only after the user presses **Search catalog**. Do not persist results or add a database migration.
- Do not run trainer binaries directly, manipulate game memory, call a private local service, or access account/session data.
- Keep existing direct/Steam launch behavior unchanged. Use neutral copy: **Trainer Catalog**, **Search catalog**, **Open trainer**, and “Trainer controls are handled by the trainer app.”
- Style the catalog as a polished trainer-library/detail experience: strong game identity, compact result rows, clear availability badges, metadata, and an obvious launch action. Reuse Dusk’s design language and retain its existing library, saves, collections, profiles, and game-management controls.
- Set package, Cargo, and Tauri versions plus the README current-version line to 1.8.20; keep the changelog vendor-neutral.

## Review Focus

1. Search and launch actions live inside Dusk’s selected-game detail panel; browsing the catalog does not open a browser.
2. Similar titles remain separate result rows. Only each individual Available result can launch.
3. Unknown/malformed status and invalid/missing IDs never yield an Open trainer action.
4. Network failure, no matches, and parse/markup failure have distinct UI states.
5. URL construction keeps arbitrary game title text in a query parameter on the fixed HTTPS origin.
6. No persisted trainer fields or account export/import behavior are added.
7. Search the user-facing app surface and release notes for provider branding before completion.

---

### Task 1: Add safe public catalog lookup and trainer URI commands

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Add serializable `TrainerCatalogResult` with `title`, optional `title_id` and `game_id`, `platforms`, optional `cheat_count`, optional `updated`, and `status` normalized to `available`, `unsupported`, `unavailable`, or `unknown`.
- Add `build_trainer_catalog_url(query: &str) -> Result<reqwest::Url, String>` using a fixed internal HTTPS origin and URL query encoding.
- Add `parse_trainer_catalog_results(html: &str) -> Result<Vec<TrainerCatalogResult>, String>` for server-rendered result rows. Recognize the public no-results marker, decode displayed HTML text safely, treat absent count/date/IDs as optional, and fail closed on page markup changes.
- Add `build_trainer_play_uri(title_id: Option<&str>, game_id: Option<&str>) -> Result<String, String>` to validate positive decimal IDs and build the internal registered play URI.
- Add Tauri commands `search_trainer_catalog(query)` and `open_trainer(title_id, game_id)`. Search should use a bounded HTTPS response and avoid blocking the UI thread. Opening must use only the validated reconstructed URI and return without waiting.
- Register both commands in the Tauri handler.

- [x] **Step 1: Add Rust tests for fixed-origin URL encoding, result parsing (Available/Unsupported/Unavailable, optional fields, multiple matches), unknown markup/status, and strict ID/URI validation.** Use representative public-page HTML fixtures, including rows with missing thumbnail IDs and blank count/date cells.
- [x] **Step 2: Run the focused Rust tests and confirm they fail before the new helpers exist.**
- [x] **Step 3: Implement catalog URL construction, bounded fetch, row parsing, safe HTML text handling, ID validation, URI reconstruction, and the two Tauri commands.**
- [x] **Step 4: Run `cargo test` and confirm the parser, URL, and URI tests pass.**

### Task 2: Render catalog results in Dusk’s game detail

**Files:**
- Modify: `src/types.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Create: `src/components/TrainerCatalogPanel.tsx`
- Create: `scripts/test-trainer-catalog-ui.cjs`

**Interfaces:**
- Expose `TrainerCatalogResult` to TypeScript with nullable IDs/count/date and a normalized status.
- Add API wrappers for `searchTrainerCatalog(query)` and `openTrainer(titleId, gameId)`.
- Add a **Trainer Catalog** section to `GameDetail`, with a user-triggered search, loading/error/empty states, and inline result cards. Show title, status, platforms, and optional count/update date.
- Show **Open trainer** only for a row marked Available with both IDs. Explain generically that trainer controls are handled by the trainer app.
- Make the section visually feel like a modern trainer-library detail view while remaining native to Dusk’s existing interface and leaving the rest of the detail panel available.

- [x] **Step 1: Create `scripts/test-trainer-catalog-ui.cjs` to exercise the actual catalog panel component and assert the user-triggered search, separate loading/error/empty states, status/platform metadata, conditional Open trainer action, and neutral trainer-controls copy. Run it and confirm it fails before the UI exists.**
- [x] **Step 2: Add the TypeScript result type and exact Tauri API wrappers.**
- [x] **Step 3: Add the GameDetail catalog section with explicit search, multiple results, disabled/busy states, safe conditional trainer launch, and vendor-neutral errors.**
- [x] **Step 4: Style the catalog cards and status states using existing Dusk detail-panel styles.**
- [x] **Step 5: Run `node scripts/test-trainer-catalog-ui.cjs` and `npm run build`.**

### Task 3: Version, changelog, and final verification

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `CHANGELOG.md`
- Modify: `README.md`

- [x] **Step 1: Set all version fields and README current-version line to 1.8.20. Add vendor-neutral changelog text for the in-Dusk catalog, result availability, and trainer launch handoff.**
- [x] **Step 2: Run `cargo test`, `node scripts/test-trainer-catalog-ui.cjs`, and `npm run build`; inspect `git diff --check` and `git status`. Search user-facing files for provider names, logos, hostnames, and URI schemes.**
- [x] **Step 3: Commit the completed change on the local `main` branch as `feat: add in-app trainer catalog`. Do not push or publish.**
