# Changelog

All notable Dusk changes are documented here.

## 1.8.19 - 2026-10-10

Keep the native window responsive during account-scope migration and cloud-state synchronization.

- Move legacy account-data copying off Tauri's window thread so large saves and screenshot folders do not block WebView2 input.
- Move cloud account-state and playtime database import/export work to blocking workers.
- Let saved guest sessions open the local library without waiting for an account-scope command; preserve ordering when guest mode is chosen during account initialization.
- Extend the Windows Edge click-through test to cover a stalled guest-scope invocation.

The Edge interaction test does not exercise the packaged WebView2 window or every Windows driver and input-device configuration.

## 1.8.18 - 2026-10-10

Prevent an unresponsive main window during background imports and slow startup.

- Never open native installer confirmations from background download completion or file monitoring.
- Allow cloud account hydration to run without blocking local app navigation.
- Add a Continue as guest escape path during initial account checking.
- Bound library-load waiting so Settings and navigation remain accessible if a local query stalls.
- Add real Windows Edge click-through smoke tests for six sidebar views and physical hit testing.
- Fix outdated download UI regression assertions; retain v1.8.17 archive integrity and extraction fixes.

This reduces hidden modal blocking but cannot yet conclusively reproduce every native WebView2 input failure.

## 1.8.17 - 2026-10-10

Make one-click multipart completion and automatic extraction retries more reliable.

- Verify downloaded multipart RAR/7z bundles with native archive integrity testing before declaring a managed transfer complete.
- Retry failed browser-downloaded archive imports after a 60-second cooldown rather than ignoring the file permanently.
- Preserve Online-Fix source information across managed download jobs for known archive-password handling.
- Add integration tests that create a real multipart 7z file, verify the full set and reject a missing final volume.
- Retain the 150 GiB extraction cap and free-disk-space checks.

Native multipart verification requires an installed 7-Zip-compatible utility; inaccessible file-host URLs remain unsupported.

## 1.8.16 - 2026-10-10

Strengthen large-game extraction preflight and verify actual multipart archives before import.

- Test browser-downloaded multipart RAR/7z archives with 7-Zip before automatic extraction.
- Detect missing, changing or duplicate volumes; retry verification when Downloads changes instead of assuming numbered parts are complete.
- Check free disk space before extraction on 7-Zip, native Windows ZIP and Python fallback paths (512 MiB reserve).
- Maintain the 150 GiB extracted-content cap, 50,000 file cap and unsafe-path protections.
- Test the archive listing's uncompressed size and reject incomplete archive fixtures.

Requires 7-Zip for automatic verification of browser-downloaded multipart archives. Host-side 404, authentication and CAPTCHA restrictions are unaffected.

## 1.8.15 - 2026-10-10

Automatic Cloudflare WARP connectivity and large-game extraction debugging.

- Use the installed official Cloudflare WARP Windows tunnel, or an existing Windows VPN profile, in Settings > VPN.
- Confirm active WARP tunnel mode rather than treating DNS-only mode as full VPN protection.
- Check VPN before game-source browsing and managed downloads; block by default if disconnected.
- Link to official Cloudflare WARP setup; keep Windows VPN connection controls and an explicit Off setting.
- Raise extracted game archive limit from 20 GiB to 150 GiB while retaining file count and unsafe-path restrictions.
- Require sequential multipart volumes before detecting downloaded archives as candidates.

Dusk does not bundle a VPN provider or bypass download-host access requirements. WARP requires a separate installation and first-run setup.

## 1.8.14 - 2026-10-10

Improve multi-volume downloads and stop wasting time on dead file-host links.

- Verify each archive volume and its public HTTPS mirror response before starting the managed bundle.
- Reject redirected 404 responses, HTML pages, and invalid archive data rather than mislabeling them as downloadable files.
- Fix archive signatures for numbered RAR/7z continuation volumes and first-volume naming variants.
- Open the original game listing inside Dusk if a download host fails after discovery.
- Run native archive regression tests during Windows CI and release builds.

Host authentication, CAPTCHA and premium-only download APIs are not bypassed.

## 1.8.13 - 2026-10-10

Add a current-website Copy link button and Automatic Windows VPN integration.

- Inject Copy link into isolated game-listing and download-host browser windows.
- Copy the current URL using a user-gesture clipboard API fallback.
- Add Settings > VPN with Automatic (default) and Off connection modes.
- Discover existing Windows VPN profiles, select one, connect on startup/browser open, and show truthful connection status.
- Open native Windows VPN configuration directly from Dusk.
- Keep credentials with Windows; no built-in VPN service/server is provided.

## 1.8.12 - 2026-10-10

Fix misleading one-click downloads caused by mirrors redirecting to 404 download endpoints.

- Probe discovered archive mirrors with a small ranged GET and verify archive magic bytes.
- Reject HTTP 404, HTML responses and other non-archive responses before starting a download.
- Validate the first and last multipart volumes and prefer a verified working mirror.
- When direct sources are inaccessible, explain the limitation and open the original listing in Dusk.
- Preserve ad/pop-up protections and installer confirmation.

## 1.8.11 - 2026-10-10

One-click downloads across multiple game listing sources, with mirror fallback and release notes visible in the update notice.

- Add Get game on Game3rb and FitGirl results to discover direct archive links.
- Retry alternate HTTPS mirrors for a file when a source fails.
- Download contiguous multipart RAR and 7z archive volumes as one background job; do not import incomplete sets.
- Extend Online-Fix Hosters file detection to multipart archives and mirror groups.
- Add native-WebView fallback when Game3rb or FitGirl HTTP search fails.
- Keep pop-up/ad blocking on while allowing user-activated navigation to recognized download hosts.
- Detect downloaded archives in Downloads subfolders and automatically extract/register supported games.
- Ask for explicit approval before running extracted game installers.
- Display release changelogs directly in Settings > Updates when a new Dusk version is available.

Limitations: Host authentication, CAPTCHA, torrent links and missing multipart volumes can prevent one-click downloads. Some archives need an installed extractor.

## 1.8.10 - 2026-10-10

Keep Home and Library game-focused and automate safe downloads in the background.

- Remove the Downloads toolbar button, dashboard and manual HTTPS archive URL form.
- Start supported Online-Fix archive transfers directly from game search, without opening a download manager.
- Poll transfer status in the background and automatically import completed single-volume archives into the game library.
- Show failure and import-result notifications rather than a Home-page download interface.
- Prevent the browser download watcher and native transfer importer from importing the same archive twice.
- Monitor matching downloads from Game3rb and offline-focused FitGirl listings in the ad-blocked Dusk browser.
- Require user approval before executing an installer; multipart game archives still need all volumes.
- Preserve per-source external browser fallback and default embedded ad blocking.

## 1.8.9 - 2026-10-10

Enable ad-filtered in-app browsing for all game listing sources.

- Open Game3rb and FitGirl listings inside Dusk's isolated webview, instead of always leaving to the system browser.
- Keep Game3rb as an alternative game-listing source and FitGirl labeled for offline-only discovery.
- Inject the lightweight ad blocker automatically in all Game3rb, FitGirl and Online-Fix browser windows.
- Block third-party pop-ups and off-site advertising redirects without hiding normal listing controls.
- Provide a separate Browser fallback button when a listing requires an external download host.
- Add tests for in-app browsing and ad filtering on both new listing sources.

## 1.8.8 - 2026-10-10

Add multiple selectable game discovery websites to Dusk.

- Add Game3rb as a searchable listing source alongside Online-Fix.
- Add FitGirl Repacks as a separate offline-focused search source.
- Search public WordPress listing pages and display results in the Dusk UI.
- Route Game3rb and FitGirl listings to the default browser with strict source URL validation.
- Keep existing Online-Fix direct archive download functionality unchanged.
- Preserve the native downloader, webview ad blocker, and cloud playtime sync.

## 1.8.7 - 2026-10-10

Fix game download buttons that do nothing.

- Get game now identifies a real Hosters game archive and starts a direct file transfer in the native Downloads panel.
- Parses and displays actual archive filenames and trusted file-host providers, distinguishing complete games from fix-only updates.
- Unblocks FileDitch, FileKeeper, Pixeldrain, Gofile and VikingFile inside the isolated download browser.
- Keeps suspicious pop-ups blocked and respects the file host's own dangerous-download confirmation.
- Shows a visible notice for blocked links instead of silently ignoring clicks.
- Removes the native downloader's 60-second total request deadline, allowing large transfers to finish.

## 1.8.6 - 2026-10-10

Add a standalone, native in-app download manager.

- Stream ZIP, RAR and 7z game archives from direct public HTTPS links inside Dusk.
- Show live byte progress, total size where known, transfer failures and cancellation.
- Download into the user's standard Downloads folder and extract/register completed archives on request.
- Reject obvious HTML/ad/invalid-file responses, unsafe paths, local/private URLs and executable downloads.
- Preserve the embedded browser for hosts that require sign-in or session-specific download links.

## 1.8.5 - 2026-10-10

Fix downloads that fail inside Online-Fix Hosters and Drive pages.

- Accept ZIP/7z/RAR files delivered through HTTPS file-hosting CDNs and supported official-site blob URLs, rather than requiring downloads to originate on the initial mirror domain.
- Attach the download handler to manual listing windows as well as automatic download windows.
- Open supported direct HTTPS archive links in the same in-app browser window.
- Avoid hiding download controls wrapped in generic ad-styled UI containers on Hosters and Drive.
- Permit manually selected fix ZIPs, while excluding them from full-game automatic registration.
- Avoid silently discarding repeated downloads of ordinary single-file game archives.
- Add a browser fallback beside each detected game mirror for host-controlled redirects or downloads.

## 1.8.4 - 2026-10-10

Correct version number in Dusk's About section.

- Replaced the hardcoded "Dusk 1.6.11" label with the actual installed app version returned by Tauri.
- The displayed version now updates automatically with future releases, without manual UI edits.
- All v1.8.3 download-link handling and embedded ad-filtering improvements are retained.

## 1.8.3 - 2026-10-10

Fix unresponsive download-host links and reduce unsafe advertising.

- Trusted Hosters and Drive buttons now navigate in the existing Online-Fix window instead of silently failing to open a new window.
- Block unsolicited new windows and off-domain advertising redirects from the embedded web browser.
- Hide recognized advertising elements, remove known ad embeds, and intercept known advertising links, including adult advertising networks.
- Keep official Online-Fix download sites accessible and preserve automatic archive selection.
- Add automated regression tests for embedded browser navigation and ad filtering.

## 1.8.2 - 2026-10-09

Automate selection of actual archive file links on supported Online-Fix hosts.

- Get game defaults to the Online-Fix Drive link for full-game archives.
- An isolated in-app browser automatically selects matching ZIP, RAR, 7z and multipart file links when accessible, including delayed page contents.
- Download handling saves accepted archive files into the standard Downloads folder that Dusk monitors.
- Prevents automatic selection of fix-only, update, advertising, and unrelated downloads.
- Login and host-gating steps remain manual; link selection does not bypass third-party restrictions.
- Keeps cloud playtime syncing introduced in v1.8.0.

## 1.8.1 - 2026-10-09

Correct game download link selection.

- Get game identifies the complete-game Hosters link from each Online-Fix article instead of opening unrelated downloads.
- Provides Online-Fix Drive as a backup mirror when the primary link is unavailable.
- Differentiates full-game links from fix-only files, torrents, and advertisements.
- Restricts link selection to verified official mirror hostnames and secure HTTPS URLs.
- Associates completed downloads with the game title, preventing unrelated archives from being imported accidentally.
- Preserves 1.8.0 cloud playtime sync.

## 1.8.0 - 2026-10-09

Cross-device playtime synchronization and Online-Fix game search repair.

- Record game hours and launch counts as distinct per-device cloud sessions, preventing progress loss when several PCs share an account.
- Preserve existing cloud playtime as a one-time historical baseline; new sessions merge across computers without double counting.
- Upload offline sessions upon reconnection and merge cloud hours back into local game stats.
- Replace unreliable Bing search with Online-Fix's native game-search results so existing listings such as How to Fish appear.
- Continue using the private, authenticated Supabase account.

## 1.7.10 - 2026-10-09

In-app game discovery, managed archive imports, and a Windows installer refresh.

- Added Online-Fix game listing search and an isolated in-app browser window, with an external-browser fallback.
- Added download-folder monitoring after opening a listing, for supported completed archives.
- Added managed ZIP, RAR, 7z, and multipart archive extraction, with 7-Zip and Python fallbacks where available.
- Added archive-path and symlink checks, extraction size/entry limits, and automatic library registration for single-executable portable games.
- Extracted installers require explicit approval before execution. Sites with gated downloads may still need manual steps.
- Builds a Windows NSIS installer for this release.

## 1.7.9 - 2026-10-09

Account login placeholder cleanup.

- Removed the hardcoded `bxane` placeholder from the Dusk username field.
- The sign-in and registration username field now uses the generic `username` placeholder.
- No account usernames, saved credentials, or profile data are changed by this update.
- Includes the lightweight Cover AI, automatic missing-cover refresh, restart-to-update flow, profile-picture persistence, and other 1.7.x improvements.

## 1.7.8 - 2026-10-09

Lightweight Cover AI for missing game artwork.

- Added a small local cover-ranking engine instead of relying on a single brittle artwork lookup.
- Generates useful title variants such as Fortnite OG -> Fortnite and Invokyr Demo -> Invokyr before searching.
- Searches Steam Store, Steam Community app search, and Epic Games Store candidates.
- Scores candidates locally using title similarity, cleaned-title similarity, detected executable company/publisher, original launcher source, and portrait-art confidence.
- Rejects low-confidence soundtrack/DLC-style mismatches and only downloads high-scoring candidates.
- Steam artwork can be selected by title even when the game was installed outside Steam.
- Web search remains fallback-only after manual, launcher-native, and local artwork fail.
- Added a dedicated startup refresh for games that still have no usable cover, independent of Dusk's 10-minute game-scan throttle.
- Missing covers are cached locally after a successful match.
- Includes the restart-to-update flow and other 1.7.x improvements.

## 1.7.7 - 2026-10-09

Restart-to-update Windows flow.

- Replaced the visible installer-based in-app update flow with a restart-to-update experience.
- Dusk downloads and SHA-256 verifies the official GitHub Release installer before restarting.
- Clicking Restart to update closes Dusk, runs the Tauri NSIS installer silently with /S, waits for installation to finish, and automatically relaunches DuskLauncher.exe.
- Normal in-app updates no longer require interacting with an uninstall/reinstall wizard.
- Added an automatic update check shortly after startup so new releases can appear without manually checking first.
- Updated the Settings UI to show Restart to update and explain the silent restart behavior.
- Local Dusk data, account data, game library state, covers, saves, and settings remain outside the replaced application binaries.
- Includes the game artwork, scanner, profile-picture, and Discord improvements from prior 1.7.x releases.

## 1.7.6 - 2026-10-09

Expanded fallback artwork lookup for missing game covers.

- Non-Steam games now search Steam by detected game title even when the game was installed from another launcher or manually.
- Strong Steam title matches use the official Steam 600x900 library artwork.
- Epic games try Epic Store tall artwork first, then Steam-by-name, then Wikidata/Commons.
- Other non-Steam games try Steam-by-name first, then Epic Store, then Wikidata/Commons.
- Web artwork remains fallback-only after manual, launcher-native, and local portrait artwork fail.
- Accepted covers are cached locally.
- Includes persistent profile pictures, stronger game detection, scanner deduplication, Discord detection mitigation, and optional Discord Rich Presence.

## 1.7.5 - 2026-10-09

Structured web fallback for missing non-Steam game covers.

- Replaced the weak Wikipedia page-thumbnail fallback with Wikidata game entities and Wikimedia Commons artwork.
- Searches by the detected game title and uses executable publisher/developer metadata when available to strengthen matching.
- Reads Wikidata P18 artwork for matched video-game entities instead of relying on arbitrary page thumbnails.
- Resolves artwork through Wikimedia Commons and rejects square or landscape images that do not fit Dusk's portrait cards.
- Keeps strict confidence checks: weaker title matches require publisher/developer agreement when metadata is available.
- Web lookup remains fallback-only after manual, launcher-native, and local portrait artwork fail.
- Accepted web covers are cached locally.
- Includes persistent profile pictures and the scanner/deduplication fixes from prior releases.

## 1.7.4 - 2026-10-09

Build fix for profile-picture persistence and artwork fallback.

- Fixed a Rust compile collision caused by two functions using the same normalized-match helper name.
- Enabled reqwest query support required by the fallback-only MediaWiki artwork lookup.
- Includes persistent Dusk profile pictures from v1.7.3.
- Includes local-first portrait artwork and non-Steam web fallback from v1.7.2.
- Keeps scanner deduplication, stronger executable detection, DuskLauncher.exe Discord mitigation, and optional Discord Rich Presence.

## 1.7.3 - 2026-10-09

Persistent Dusk profile pictures.

- Dusk now saves each account avatar's stable private-storage path in the authenticated account metadata.
- Profile pictures persist reliably across app restarts, sign-outs, and later sign-ins.
- Signed avatar URLs are regenerated from the saved private object path instead of rediscovering the file on every launch.
- Existing avatars from earlier Dusk versions are detected once and migrated automatically to the persistent avatar-path format.
- Avatar signed URLs now last up to seven days while the underlying private avatar remains permanently stored until the user removes it.
- Removing an avatar deletes the private storage object and clears the saved account metadata path.
- Keeps the local-first artwork lookup and scanner fixes from v1.7.2.

## 1.7.2 - 2026-10-09

Fallback-only web artwork lookup for non-Steam games.

- Keeps Dusk local-first: manual covers are preserved, launcher-native/local portrait art is preferred, and web lookup only runs when those sources fail.
- Added non-Steam web artwork fallback using title plus detected publisher/company metadata when available.
- Uses Wikipedia/MediaWiki as a keyless metadata source instead of arbitrary image scraping.
- Requires strong title matching, video-game context, portrait-oriented artwork, and extra publisher/company agreement for weaker title matches.
- Rejects ambiguous, landscape, square, tiny, or non-HTTPS image results instead of guessing.
- Caches accepted fallback artwork locally so Dusk does not re-download it on every launch.
- Steam keeps its official portrait library-art fallback, but only after local Steam cache and local install-folder artwork fail.
- Existing scanner deduplication, portrait-art filtering, and manual-cover preservation from v1.7.1 remain included.

## 1.7.1 - 2026-10-08

Game library cleanup and portrait artwork hotfix.

- Replaced wide Steam header artwork with portrait Steam library artwork suited to Dusk game cards.
- Prefer locally cached Steam 600x900 library artwork before downloading an official Steam portrait fallback.
- Local artwork detection now favors covers, posters, key art, box art, capsules, and vertical images while rejecting wide headers, heroes, banners, logos, and icons.
- Added scanner deduplication by executable/install location with launcher manifests preferred over generic device-folder matches.
- Existing stale device-scan duplicates that match Steam, Epic, or GOG installs are hidden on the next scan.
- Added cover-origin tracking so user-selected manual covers are preserved while old automatic artwork can refresh to the improved portrait format.
- Keeps the stronger executable filtering and expanded Windows library scanning from v1.7.0.

## 1.7.0 - 2026-10-08

Stronger installed-game detection and automatic per-game artwork.

- Expanded automatic Windows game-folder discovery to include common Xbox, EA, Ubisoft, portable, and custom game-library locations.
- Tightened executable selection so Dusk is less likely to pick uninstallers, anti-cheat helpers, redistributables, updaters, launch helpers, or service executables as the game binary.
- Added automatic local artwork discovery using installed cover, poster, key art, capsule, header, hero, banner, and library images when available.
- Steam games now automatically use the game's own official Steam header artwork when no user-selected cover already exists.
- Existing manually selected covers are preserved and never overwritten by automatic artwork.
- Detection remains local-first; artwork falls back gracefully when a launcher or game does not expose usable assets.

## 1.6.11 - 2026-10-08

Optional Dusk Discord Rich Presence.

- Added optional Discord Rich Presence support using Discord IPC.
- Rich Presence is disabled by default and never connects unless the user enables it.
- Added a Discord Rich Presence card in Settings with a Discord Application ID field.
- When enabled, Dusk identifies itself intentionally as the Dusk desktop launcher instead of relying on generic process detection.
- Presence states include Browsing library, Viewing screenshots, Viewing achievements, Adjusting Dusk settings, and Launching a game.
- Dusk uses a non-game-style Watching activity type rather than publishing itself as a played game.
- Disabling Rich Presence clears the activity and closes the Discord IPC connection.
- Dusk still cannot override Discord's separate Registered Games process scanner; users can independently disable Dusk there in Discord.
- Includes the v1.6.10 live window/taskbar crescent icon and DuskLauncher.exe process identity changes.

## 1.6.10 - 2026-10-08

Windows app icon and Discord detection cleanup.

- Explicitly sets the live Dusk window/taskbar icon from the official crescent PNG at runtime.
- Enabled Tauri PNG decoding and the window set-icon permission for the packaged app.
- Keeps the crescent embedded in the executable, shortcuts, installer, and uninstaller.
- Renamed the main Windows process binary from dusk.exe to DuskLauncher.exe while keeping the installed product, shortcut, and UI name as Dusk.
- Updated CI and release startup tests for the new executable filename.
- Dusk has no Discord RPC integration and does not publish Discord activity itself; the executable rename is intended to reduce Discord's automatic false game detection.

## 1.6.9 - 2026-10-08

Official transparent crescent branding.

- Replaced the previous Dusk desktop mark with the user-approved crescent logo.
- The in-app logo now uses a smooth transparent SVG instead of a raster tile.
- Removed the artificial dark/violet background behind the in-app logo.
- Added official violet, cyan, and red transparent SVG variants under the brand folder.
- Added generated violet, cyan, and red transparent PNG variants to the repository.
- Regenerated Windows icon sizes and icon.ico from the same crescent geometry.
- Regenerated installer and uninstaller header/sidebar artwork with the new crescent.
- Added subtle cyan and red installer accent rails while keeping violet as Dusk's primary color.
- Added a Brand Assets workflow that regenerates and commits raster/Windows assets when the vector branding changes.
- App, title-bar, login/register screen, executable, shortcuts, installer, and uninstaller now share the same crescent identity.

## 1.6.8 - 2026-10-07

Profile UI cleanup.

- Removed the Profiles item from the main sidebar navigation.
- Removed the standalone Profiles page.
- Removed the bottom owner-profile dropdown and add-profile button from the sidebar.
- Existing owner-profile data, libraries, saves, backups, and switching logic are preserved.
- Profile management remains available in Settings, so no existing data is deleted.

## 1.6.7 - 2026-10-07

Custom Dusk account avatars.

- Added user-uploaded profile avatars for signed-in Dusk accounts.
- Avatar uploads accept image files up to 150 MB.
- Added a private Supabase Storage bucket dedicated to Dusk avatars.
- Bucket-level file-size and image MIME-type restrictions enforce the avatar upload rules server-side.
- Each user can only read, upload, replace, and delete the avatar stored under their own authenticated user ID.
- Avatar images are displayed as round profile pictures using a centered square crop with object-fit cover.
- Added upload, change, and remove avatar controls to Dusk account Settings.
- Added the signed-in account avatar, display name, and username to the Dusk sidebar.
- If no avatar exists, Dusk falls back to the account's initial.
- Avatar/name changes update the sidebar immediately without restarting Dusk.
- Avatar files remain private and are displayed through short-lived signed URLs.

## 1.6.6 - 2026-10-07

Password recovery for Dusk accounts.

- Added a visible Forgot password? action to the Dusk sign-in screen.
- Users can enter the email attached to their Dusk account and request a Supabase recovery email.
- Recovery requests use generic messaging so Dusk does not reveal whether an email address has an account.
- Added a hosted dark-violet Dusk password-reset page.
- The recovery page validates the temporary recovery session before changing the password.
- New passwords must be at least 8 characters and must be confirmed before submission.
- After resetting, the user signs in normally with the same Dusk username and the new password.
- The reset page does not expose the Supabase service-role key and does not store passwords.
- The hosted recovery flow avoids using the old localhost page as the intended reset destination.

## 1.6.5 - 2026-10-07

Editable Dusk accounts and owner profiles.

- Added editable Dusk account display names and usernames.
- Added editable account email addresses.
- Added secure password changes that require the current password.
- Passwords remain managed by Supabase Auth and are never stored by Dusk.
- Username changes keep username login working with the new username.
- Email changes update the authenticated Dusk account immediately without the old localhost confirmation redirect.
- Registration now asks for a display name as well as email, username, and password.
- Added a dedicated account-profile editor in Settings with separate Profile and Security sections.
- Added rename controls for local owner profiles on both the Profiles page and Settings.
- Owner-profile renames sync through Dusk account state.
- Account-table writes are restricted to the validated server-side account-update endpoint; the desktop client only receives read access.
- Includes the persistent per-profile achievements introduced in the 1.6.4 code line.

## 1.6.4 - 2026-10-07

Persistent, profile-owned achievements.

- Achievements are now saved in Dusk instead of being display-only calculations.
- Achievement progress is stored separately for each owner profile.
- Saved progress uses a high-water mark, so earned progress never moves backwards if games, screenshots, or local history are removed later.
- Once an achievement is unlocked, it stays unlocked.
- Achievement progress and unlock state are included in Dusk account sync and merge across devices.
- Cross-device merges keep the highest progress/unlocked state instead of overwriting it with lower progress.
- Existing profiles automatically create saved achievement records from their current Dusk stats the next time achievements are loaded or account state is synced.
- Deleting a profile also removes only that profile's saved achievement records.

## 1.6.3 - 2026-10-07

Profiles are now directly available from the main Dusk navigation.

- Added a dedicated Profiles page to the sidebar.
- Added visible cards for every owner profile.
- Shows the active profile, profile count, current library size, per-profile game count, backup count, and last-used date.
- Switch profiles directly from the Profiles page.
- Create new profiles from the page header or the dedicated new-profile card.
- Delete non-required profiles directly from the page.
- Open the active profile's library with one click.
- Preserved the compact profile switcher in the sidebar and the advanced profile controls in Settings.
- Profiles continue to keep owner libraries, collections, save folders, backup vaults, and synced account state separate.

## 1.6.2 - 2026-10-07

Registration, updater, and logo hotfix.

- Removed the broken browser-based email confirmation redirect that pointed to localhost.
- New Dusk accounts are verified server-side and signed into the desktop app immediately.
- Added a Dusk-branded Supabase confirmation email template to the repository for future confirmation flows.
- Replaced the optional Tauri updater-plugin runtime path with a built-in GitHub Releases updater.
- Dusk now checks the latest release, compares versions, downloads the official Windows x64 installer, verifies the GitHub SHA-256 digest when available, launches the installer, and closes the current app.
- Removed the misleading "plugin updater not found" failure path.
- Fixed the official Dusk logo pipeline to use the full approved emblem instead of cropping away the left side of the D.
- Kept guest mode, account sync, custom window controls, and the branded installer.

## 1.6.1 - 2026-10-06

Account-screen and desktop hotfix.

- Restored minimize, maximize/restore, close, and drag controls while the login/register screen is open.
- Added a persistent Continue as guest path for users who want a local-only Dusk library without an account.
- Added a Settings path from guest mode back to sign-in.
- Fixed desktop Supabase configuration fallback so account login works even when release environment variables are missing.
- Kept only the public Supabase publishable key in the desktop client; privileged server credentials remain server-side.
- Cropped the approved Dusk emblem for UI, Windows icons, shortcuts, installer, and uninstaller so the logo is clearly visible.
- Disabled cloud-sync actions while running as a guest.

## 1.6.0 - 2026-10-05

Dusk accounts and cross-device account state.

- Added the official violet Dusk desktop logo to the title bar, sidebar, account screen, loading screen, executable, shortcuts, and About panel.
- Added a custom dark-violet NSIS installer with branded header/sidebar artwork and matching installer/uninstaller icons.
- Added a deterministic Windows branding pipeline that generates icon and installer assets from one approved logo source.
- Added a custom Dusk login and registration screen.
- Registration now uses email, username, and password; login uses username and password.
- Added Supabase-backed Dusk accounts with unique usernames and persistent sessions.
- Replaced anonymous per-install cloud identities with authenticated account-owned save storage.
- Added cross-device sync for owner profiles, library membership, favorites, playtime, launch counts, collections, and active profile.
- Added safe local merge behavior so device-specific executable paths and save-folder paths are never copied blindly from another PC.
- Added account sign-out from Settings.
- Added private per-account state storage with Row Level Security.
- Added a username login Edge Function that resolves usernames server-side without exposing privileged keys to the desktop client.
- Preserved private physical save-backup uploads under the authenticated user's Storage namespace.

## 1.5.0 - 2026-10-05

Installer hub and bxane branding.

- Added a safe local game-installer flow for user-selected .exe and .msi files.
- Added one-click browser links for Steam, Epic Games, GOG, and itch.io.
- Added a persistent clickable bxane creator pill linking to https://guns.lol/bxane.
- Physical owner save-file vaults: each profile stores actual save files separately for every configured game.
- Added Save to profile / Load profile files controls with pre-load safety backups.
- Updated package authorship/publisher branding to bxane.
- Dusk does not automatically download or install games from unofficial redistribution sites.
- Preserves automatic game discovery, Downloads scanning, hidden scan helper processes, custom window controls, and rapid-click stability protections from earlier releases.

## 1.0.4 - 2026-10-05

Desktop window and scan UX hardening.

- Replaced the native frame with a Dusk custom title bar.
- The default window is larger and centered on screen.
- The top title area can be dragged to move the window.
- Added working minimize, maximize/restore, and close controls.
- Double-clicking the draggable title area toggles maximize/restore.
- Internal Windows helper commands used for scanning and process tracking now run with CREATE_NO_WINDOW, eliminating flashing console windows.
- Downloads and Downloads/Games are now included in the default bounded device scan roots.
- Loose installer executables in Downloads are not blindly imported; Dusk scans contained game folders conservatively.
- Preserves v1.0.3 automatic background scanning and v1.0.2 rapid-click stability protections.

## 1.0.3 - 2026-10-05

Automatic discovery and responsiveness hardening.

- Dusk now automatically scans for games shortly after startup.
- Automatic rescans run about every 10 minutes while Dusk remains open and when the app becomes active after a stale scan.
- Added bounded discovery for common game folders across available Windows drives and common profile game folders.
- Device-folder scans run on blocking workers and use strict time, folder-count, depth, and entry limits.
- Added a Settings toggle for automatic device scanning plus last-scan status.
- Added a Device source/filter for games found outside supported launcher manifests.
- Manual and automatic scans share one hard lock so repeated clicks cannot stack scan jobs.
- Preserved the v1.0.2 anti-freeze changes: coalesced refreshes, click deduplication, SQLite busy handling, lazy local media loading, and background filesystem work.

## 1.0.2 - 2026-10-05

Stability and responsiveness hotfix.

- Database schema/migration initialization now runs once per process instead of on every command.
- Added SQLite busy timeout handling for short-lived concurrent access.
- Heavy game scans, screenshot scans, save backups, and save restores now run on blocking workers instead of tying up command handling.
- Rapid repeated clicks are deduplicated for scans, launches, favorites, collections, update checks, screenshot deletion, fullscreen changes, and game-detail actions.
- Core library refreshes and screenshot refreshes are coalesced instead of stacking overlapping requests.
- Favorites update optimistically without forcing a full database/library reload on every click.
- Repeated screenshot-page visits no longer reload all screenshot previews unnecessarily.
- File picker and add-game actions now have hard re-entry locks.
- Controller polling no longer recreates its animation loop when connection state changes.
- Toast timers no longer let older notifications clear newer ones.
- Media previews now load directly from Dusk's scoped local asset storage instead of sending full image files as base64 through IPC, sharply reducing memory and UI pressure.

## 1.0.1 - 2026-10-05

Startup hotfix.

- Fixed a Windows startup exit caused by registering the Tauri updater plugin in builds that did not contain updater signing/public-key configuration.
- Updater runtime registration is now injected only for properly signed updater builds.
- Added a CI smoke test that launches the built Windows executable and verifies it remains running.
- Pinned Tauri updater JavaScript/Rust versions to matching releases.

## 1.0.0 - 2026-10-05

First stable release.

### Library and launching
- Steam installed-game discovery from local manifests and library folders.
- Epic Games discovery from local manifests.
- GOG discovery from Windows install records.
- Common emulator detection.
- Manual Windows executable entries.
- Native launching and tracked play sessions.
- Search, filtering, sorting, favorites, and collections.

### Media and personalization
- Custom game covers.
- Automatic screenshot discovery for Steam, game folders, Windows screenshots, and Xbox Game Bar captures.
- Duplicate-safe local screenshot gallery.
- Night, OLED, and Slate themes.
- Violet, Ember, and Cyan accents.

### Saves and safety
- Per-game save-folder configuration.
- Manual restore points.
- Pre-restore safety snapshots.
- Protection from selecting broad system/profile folders as save roots.

### Controller and console mode
- Standard Gamepad API navigation.
- D-pad / stick directional focus.
- Controller select, back, and launch shortcuts.
- Native fullscreen console mode.
- Larger controller-friendly UI and focus states.

### Desktop and release
- Tauri 2 + React + TypeScript + Rust.
- Local SQLite persistence.
- NSIS Windows installer.
- Windows CI for frontend and Rust builds.
- Optional signed updater artifacts through Tauri.
- Optional Windows Authenticode signing when a certificate is configured.

