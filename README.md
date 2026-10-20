# Dusk

Dusk is a local-first desktop game and mod manager built with Tauri, Rust, React, TypeScript, and SQLite.

**Current version: 1.0.0**

It scans real local game installations, launches games, tracks sessions it can observe, and keeps the library database on the user's PC. Dusk does not ship with fake games or seeded playtime.

Windows installers include automatic game scanning, provider launching, process-based playtime tracking, and mod deployment. Linux packages are available for manual game entries and direct launching; automatic scanning, provider launching, process-based playtime tracking, mod deployment, Riot support, and memory trainers are currently Windows-only.

The in-app Dusk Catalog includes 3,510 unique game trainer listings and works offline. Search titles, compare platform support, and review availability details.

## Games-first Home and background downloads (v1.8.10)

Home and Library now focus on game content. Dusk's old **Downloads** toolbar
button, dashboard, and manual HTTPS URL-paste form have been removed.

Select **Get game** from Online-Fix search to start a supported full-game
archive in the native downloader automatically. The transfer runs while Dusk
is open, even when you navigate to Home or Library. Dusk tracks completion,
reports failures through notifications, and safely extracts complete,
single-volume archives into the managed game library. Game cards appear
after the importer detects a launchable game. Downloads are still restricted
to secure file sources; access-gated or login-dependent hosts need their normal
browser interaction.

For Game3rb and FitGirl (offline discovery), use **Open in Dusk** to browse
with ad blocking already enabled. Dusk watches for an appropriately named
completed archive in Downloads and imports it when possible. Browser fallback
is still available. Filenames that don't identify the selected game reliably
cannot be auto-imported; choose **Import archive** in that case.

Multipart archives (RAR/7z split volumes) require all matching volumes before
import. Dusk does not run untrusted installers automatically; executable-only
archives require your confirmation and possibly manual game registration.

## In-app game browser and default ad blocking (v1.8.9)

Choose **Web search** in Dusk, then select **Online-Fix**, **Game3rb**,
or **FitGirl · Offline only**. Search results appear inside Dusk.
**Open in Dusk** on a Game3rb/FitGirl result opens a separate, isolated
in-app browser window; **Browser fallback** is available for pages needing
another website or service.

The embedded browser's lightweight ad filter is **on by default** for
all three sources and for supported Online-Fix file-host pages. Known
advertising embeds, advertising links and pop-up windows are blocked.
The site browser only navigates within its selected original website
to reduce malicious ad redirects. Third-party file hosts can be
opened separately with Browser fallback. The filter cannot guarantee
that every ad is blocked, and it does not protect the system browser.

FitGirl is labeled an **offline-only discovery source**; this does not
imply every game requires or supports offline operation. The downloader
does not bypass source login, authorization, or download-host restrictions.

## Features

- Per-game mod profiles with local import, Thunderstore downloads, dependency installs, game-specific loader routing, and reversible deployment
- Offline Dusk Catalog with 3,510 searchable game trainer listings
- Steam library scanning from local app manifests and library folders
- Epic Games scanning from local manifest files
- GOG scanning from local Windows install records
- Riot game discovery for League of Legends and VALORANT
- Common emulator detection (Dolphin, PCSX2, RetroArch, Ryujinx, Cemu, PPSSPP, DuckStation)
- Automatic background game scanning on startup and at safe intervals
- Bounded common game-folder discovery across available Windows drives and Downloads
- Custom draggable Windows title bar with minimize, maximize/restore, and close controls
- Password recovery by email with a hosted Dusk reset page
- Private account avatars with user uploads up to 150 MB and round in-app display
- Official violet Dusk desktop logo across the app, executable, shortcuts, and installer
- Optional Discord Rich Presence for an intentional Dusk launcher identity
- Custom dark-violet NSIS installer artwork with branded header/sidebar and matching install/uninstall icons
- Hidden background Windows scan helpers with no flashing terminal windows
- Safe local installer hub for user-selected EXE/MSI installers
- Official store shortcuts for Steam, Epic Games, GOG, and itch.io
- Clickable bxane creator pill linking to https://guns.lol/bxane
- Physical per-owner save-file vaults for each configured game
- Manual game entries for any local Windows executable
- Native game launching
- Steam launch through the Steam protocol
- Playtime tracking for known game processes, including games opened outside Dusk while it runs in the tray
- Favorites
- Search, source filtering, and sorting
- Custom collections
- Custom cover images
- Automatic screenshot discovery from Steam, game folders, Windows Screenshots, and Xbox Game Bar Captures
- Imported local screenshot gallery with duplicate protection
- Per-game save-folder backups and restore points
- Automatic pre-restore safety snapshots
- Controller navigation using the standard Gamepad API
- Fullscreen console mode with controller focus and shortcuts
- Dusk achievements calculated from real local Dusk data
- Local stats dashboard
- Night, OLED, and Slate themes
- Violet, Ember, and Cyan accents
- SQLite persistence
- Dusk account login with cross-device library/progress sync

## Mod manager (v1.11.0)

Open a game's details to browse its Thunderstore community, download a mod and
its declared dependencies, or import an extracted local folder. Dusk uses the
game-specific loader and install rules in the bundled Thunderstore ecosystem
catalog. It supports the common package layouts and loader packages declared
there, including BepInEx and MelonLoader. Dusk does not run arbitrary EXE/MSI
mod installers.

Mod files are installed into Dusk's local profile library first. Enabling a mod
deploys them into the game folder with backups; disabling restores files only
when they still match Dusk's deployment receipt. Profile switching follows the
game's stable Dusk library identity, so moving an installation carries its
profiles and lets Dusk restore the moved files. If both old and new folders
exist, Dusk asks you to resolve that duplicate instead of guessing.

Declared package dependencies are installed before the package that needs
them, and Dusk prevents disabling a dependency while an enabled package needs
it. Conflicting target files are reported and never silently overwritten.
The game loader remains responsible for runtime plugin order. For BepInEx,
plugin dependency metadata determines which plugins load first; Dusk applies
Thunderstore package dependency order and game-specific install routing.
Mod profiles and backups stay on this PC.

The bundled Thunderstore game and install-rule catalog is MIT-licensed data
from r2modmanPlus; see `THIRD_PARTY_LICENSES/r2modman-MIT.txt`.

## Stack

- Tauri 2
- Rust
- React 19
- TypeScript
- Vite
- SQLite via rusqlite

## Cloud-synced hours played (v1.8.0)

Signed-in users' hours played and launch counts are synchronized to Supabase.
Dusk imports prior 1.7.x cloud totals once, then uploads distinct play sessions
identified by device and local session ID. Devices merge recorded totals without
deleting other devices' sessions; offline play is uploaded when cloud access
returns. Local game executable paths stay on the PC.

Guest accounts remain local-only. Session duration is counted only when Dusk
can observe the launched process. Sync runs automatically when refreshing the
library and periodically while online.

## Multiple game listing sources (v1.8.8)

Enable **Web search** in the top bar and select one of three discovery sources
from the **Source** menu:

- **Online-Fix · Online:** Online game listings with the existing Hosters
  native download option when a direct archive link exists.
- **Game3rb · Games:** Live search of Game3rb public game listings.
  Opening a result uses the system browser for that site's own downloads.
- **FitGirl · Offline only:** Search the official FitGirl Repacks listing
  site in an offline-focused category in Dusk. Opening a result uses
  the system browser.

The search backend parses the selected site's real WordPress result cards and
restricts resulting links to exactly that site's official hostname. It
does not assume the two newly added sources expose an automated
download API, bypass logins or third-party restrictions, or run installers
without consent.

## One-click game downloads (v1.8.7)

The **Get game** button now retrieves the actual archive choices displayed by
the Online-Fix Hosters file listing, instead of just opening a Drive/Hosters
landing page and waiting for a WebView2 popup. Where Hosters exposes a direct
full-game archive, Dusk starts the file in its **native Downloads** manager
immediately and displays its transfer progress. Fix/repair/update archives
are distinguished from full-game files and are never auto-selected. The other
file providers remain visible as alternate choices.

Trusted file hosts such as FileDitch, FileKeeper, Pixeldrain, Gofile and
VikingFile may be opened from the embedded browser rather than silently
blocked. The site-provided safety confirmation is never auto-accepted.
Unsupported/ad destinations are blocked with a visible notice.

Direct file transfers no longer fail after a fixed 60-second total timeout:
the fixed overall request deadline is disabled; connection setup still has a timeout. Browser-only
and authenticated downloads still require the site's normal interaction.
When a provider rejects the native downloader, Dusk displays the transfer
error and the browser fallback remains available. No downloaded installer is
executed without separate confirmation.

## Native download engine (introduced in v1.8.6)

The native HTTPS archive transfer engine remains available in the background,
without a separate Download Manager screen. It validates public HTTPS URLs,
rejects known advertising networks, local-network addresses and invalid
archive signatures, and limits each file to 100 GiB. Transfers run while the
launcher is open. Failed or cancelled transfers remove incomplete files.

Only direct file URLs are supported by the native transfer engine. Hosts
requiring login, browser cookies, or user confirmation still need those
steps in Dusk's isolated browser or its external-browser fallback.

## Working downloads from Hosters, Drive and HTTPS CDNs (v1.8.5)

Dusk no longer assumes every actual archive comes from the initial Drive or
Hosters host. Once you click a genuine game archive, WebView2 may download
it through a secure third-party storage CDN, a signed URL, or an official
site-generated blob. Dusk now accepts these downloadable ZIP, RAR and 7z
archives in the embedded browser and saves them in Downloads. It keeps the
site navigation filter for known ad pages, denies executable downloads, and
never runs an installer without consent.

Manual fix/repair/update ZIP files can also be downloaded, but Dusk will
**not** automatically register them as a full game. Repeated single-file
archives are given unique filenames instead of silently failing. Automatic
imports continue to match the selected game by title.

If a particular host's login or download control requires a separate browser,
use the **Browser fallback** button beside that specific Drive/Hosters mirror.
Some hosts prevent embedded downloads or use redirects Dusk intentionally
cannot trust; they may require completing the download in your browser.

## Online-Fix browser click repair and ad blocking (v1.8.3)

The isolated in-app Online-Fix browser now opens trusted Hosters and Drive
`target="_blank"` links in the current Dusk download window. A lightweight
ad/content filter hides recognizable advertisements, strips known ad embeds and
blocks known ad-network link clicks. Native navigation and popup handlers deny
external redirects and new ad windows. It also tries to hide adult-content
advertising loaded on the game listings, without blocking official game links.

This protects **Dusk's in-app Online-Fix browser only**, not other apps,
a normal web browser, or every ad request. It is not a full system-wide
network ad blocker. Third-party download hosts not explicitly allowed in Dusk
can still be opened using the external browser fallback.

## Automatic archive link selection (v1.8.2)

**Get game** now opens the listed game's Online-Fix Drive source and enables a
conservative auto-selector in a separate, isolated Dusk window. When the host
makes ordinary game archive links available, Dusk selects matching ZIP, RAR,
and 7z parts, and saves supported WebView downloads into the user's standard
Downloads directory. The existing download watcher imports files after they
have completed.

Dusk does not click advertisements, complete content-locker tasks, bypass
authentication, or guess URLs. Some hosts hide downloads behind JavaScript,
redirects, or unsupported file listings; in those cases finish the site's steps
or select the download manually. The auto-selector only runs on exact official
Drive and Hosters domains, and it ignores fixes, updates, unrelated filenames,
and unsafe files. Auto-selection is best-effort and may require the external
browser fallback if the site rejects embedded browsing.

**Online-Fix Drive is the default for full games.** Hosters may offer only
fixes and updates. Use Hosters only when the page clearly offers the full game.

## Selecting the correct full-game download (v1.8.1)

In the Online-Fix search results, **Get game** examines the selected listing's
game-content section and identifies the actual full-game links. It selects
the **Online-Fix Hosters** mirror first and shows the **Online-Fix Drive**
mirror as another choice. These URLs are checked against exact source hosts;
fix-only downloads, torrents and unrelated advertisements are never selected
as the standard game download.

Select the preferred source and complete any required authentication,
download-host interaction or download in the site. Dusk then monitors
the standard Downloads folder for an archive whose filename matches
the selected game, instead of importing unrelated recent downloads.
Installers are still launched only after the user confirms.

## In-app game discovery and managed archive import

Dusk has an **Online-Fix** search toggle beside the library search box. Search
results appear inside Dusk, and **Open in Dusk** opens the selected listing in
an isolated webview window. **Browser fallback** uses your normal browser if
a third-party download host does not work in the embedded window.

Opening a listing starts a **two-hour download monitor**. While Dusk is open,
it checks the current Windows user's `Downloads` folder for new, completed
ZIP, RAR, and 7z archives. It waits for file size to stabilize before attempting
automatic import. Multipart archives are supported when all parts are available
in the same folder; the first `.part1.rar` or `.7z.001` file starts extraction.
If you download to a different folder, use **Import archive** and pick the file
manually.

Archives are extracted into `<Dusk app data>/managed-games/<game>-<id>`.
Dusk prefers an installed **7-Zip** executable; on Windows ZIPs also have
a PowerShell/.NET fallback. An installed **Python 3** interpreter provides
another fallback: ZIP needs only the standard library, 7z needs `py7zr`,
and RAR needs `rarfile` and an extraction backend such as `unrar`.
For monitored Online-Fix downloads, the documented archive password is
used automatically. Dusk rejects unsafe archive paths and symlinks and applies
a 20 GiB / 50,000-entry extraction limit.

If the archive contains exactly one plausible portable-game executable, Dusk
adds it directly to the library. For installers, Dusk requires an explicit
confirmation before executing any installer, and it does not bypass Windows
security prompts. Multiple possible executables require manual selection.

**Limitations:** Third-party sites may require user actions, login, an ad
unlock, or a torrent client before any archive exists in Downloads.
Dusk does not bypass these steps, download arbitrary gated files, or
automatically execute untrusted programs. The monitor only runs while Dusk
is open.

## Run in development

Requirements:

- Node.js
- Rust stable
- Windows WebView2 / Tauri Windows prerequisites

Commands:

    npm install
    npm run tauri dev

## Build the Windows installer

    npm install
    npm run tauri build -- --bundles nsis

## Build Linux packages

Install Tauri's Linux system dependencies, then run:

    npm install
    npm run tauri build -- --bundles deb,appimage

The official logo source is `src/assets/dusk-logo.svg`. Windows regenerates the NSIS artwork and icons before a build; Linux uses the checked-in app icon assets.

## Data

Dusk stores its SQLite database, copied cover images, and imported screenshots in the Tauri application data directory for dev.bxane.dusk.

Playtime is only recorded for sessions Dusk can actually track. If a Steam game's executable cannot be detected, Dusk can still request the Steam launch but explicitly reports that the session cannot be counted.

## Automatic detection

Current scanners:

- Steam
- Epic Games
- GOG
- Common emulator installations

Detected emulators are added as launchable library entries. ROM-specific launching will be handled separately; any other Windows game can still be added manually by selecting its executable.

## Screenshot detection

Dusk automatically checks:

- Steam userdata screenshot folders and matches them by Steam App ID
- Common `Screenshots` / `Captures` folders inside known game install directories
- `Pictures\Screenshots`
- `Videos\Captures` used by Xbox Game Bar

For shared Windows folders, Dusk only imports an image when the filename contains a sufficiently specific known game title. Imported source paths are tracked so repeated scans do not create duplicates. If an automatically found screenshot is removed from Dusk, its original source file is left untouched and Dusk remembers not to re-import it automatically.

## Dusk accounts, owner profiles, and Supabase cloud saves

Dusk uses Supabase-backed accounts. Registration uses email, username, and password; normal sign-in uses username and password. Each account can contain multiple owner profiles, and each owner has a separate visible game library, save-folder configuration, and backup set.

When Supabase cloud saves are configured, Dusk uploads the **actual save files** from each backup to the private `dusk-savefiles` Storage bucket. The object structure is:

```text
<supabase-user-uuid>/
  profiles/<profile-id>/
    games/<game-id>/
      backups/<backup-id>/
        <original relative save-file paths>
        _backup.json
```

Dusk also syncs account-owned metadata including profiles, library membership, favorites, playtime, launch counts, collections, and the active profile. Executable paths and save-folder paths remain device-local so a second PC never inherits invalid filesystem locations. Local backups remain usable if cloud access is temporarily unavailable.

Cloud setup requires:

- the Dusk Supabase project URL
- the Dusk Supabase publishable key
- the private bucket/RLS policies in `SUPABASE_STORAGE_SETUP.sql`

Only a publishable key is used in the desktop client. A Supabase `service_role` or secret key must never be embedded in Dusk.

## Owner profile save files

Each local owner profile can keep its own physical copy of a game's actual save files.

For every configured game, Dusk stores the active owner's current copy under:

`<Dusk app data>/profiles/<profile-id>/games/<game-id>/current/`

The game detail panel provides:

- **Save to profile** — copies the live game save folder into the active owner's physical vault.
- **Load profile files** — creates a safety backup of the current live saves, then replaces them with that owner's stored files.
- File count, total size, and last-updated information.

Choosing a save folder seeds the owner's vault with the current live files. Creating a normal manual backup also refreshes that owner's current vault.

Profile deletion removes that owner's Dusk-managed vault and backup copies, but never deletes the game's live save folder.

## Save-game backups

Save backups are opt-in per game. Dusk does not guess save locations.

From a game's details panel:

1. Choose the exact save folder used by that game.
2. Create a restore point with **Back up now**.
3. Review backup time, file count, and size.
4. Restore any snapshot when needed.
5. Delete old Dusk snapshots without changing the live save folder.

Before every restore, Dusk first creates a `pre-restore` safety backup of the current saves. Restore operations replace the contents of the configured save folder with the selected snapshot.

Dusk rejects drive roots and broad Windows, Program Files, user-profile, and AppData roots as backup targets. Backups stay local under Dusk's application data directory.

## Controller and console mode

Dusk supports standard gamepads through the browser Gamepad API used by the Tauri webview.

Default controls:

- D-pad / left stick: move focus
- A / Cross: select the focused control
- X / Square: launch the focused game card
- B / Circle: go back / close the current panel
- Menu / Start: toggle fullscreen console mode
- F11: keyboard shortcut for console mode

Console mode switches the Tauri window into native fullscreen, collapses Dusk to an icon rail, increases card sizing and focus visibility, and shows an on-screen controller hint bar. The setting is stored locally.

## CI and releases

GitHub Actions checks the React build and Rust code on Windows.

Push a `dusk-v*` tag to build a Windows NSIS installer and Linux `.deb` and AppImage packages, then publish them as a GitHub Release with notes from `CHANGELOG.md`. Windows Authenticode signing is enabled when the certificate secrets are configured; otherwise the Windows installer is unsigned.

## Stable release

Stable installers are published on GitHub Releases:

- Repository: https://github.com/bxanedot/dusk
- Releases: https://github.com/bxanedot/dusk/releases
- Changelog: `CHANGELOG.md`
- Signing setup: `RELEASE_SIGNING.md`

The release workflow never commits private signing material. Missing signing secrets cause the corresponding signing layer to be skipped rather than fabricated.

## Contributing

Contributions, forks, fixes, and feature work are welcome. See CONTRIBUTING.md.

## License

Dusk is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for the complete terms.


### Windows process identity

Dusk installs and appears to users as **Dusk**, but the Windows process binary is **DuskLauncher.exe**. This avoids using the generic `dusk.exe` process name that Discord can falsely auto-detect as a game.
