# Dusk Auto-Updater Setup

Dusk v1.0.0 includes the official Tauri updater integration.

The updater uses GitHub Releases and a signed `latest.json` manifest. Update signatures are mandatory in Tauri and the private signing key must never be committed to this public repository.

## One-time signing-key setup

Run these commands on a trusted Windows PC from the Dusk repository:

```powershell
npm install
New-Item -ItemType Directory -Force "$HOME\.tauri" | Out-Null
npx tauri signer generate -w "$HOME\.tauri\dusk.key"
```

Keep `$HOME\.tauri\dusk.key` private and backed up securely.

The signer prints the public key. The private key content can be read with:

```powershell
Get-Content -Raw "$HOME\.tauri\dusk.key"
```

## GitHub Actions secrets

Open the Dusk repository:

Settings -> Secrets and variables -> Actions -> New repository secret

Create:

- `TAURI_SIGNING_PRIVATE_KEY` — complete contents of `dusk.key`
- `TAURI_SIGNING_PUBLIC_KEY` — public key printed by the Tauri signer
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — only if you protected the key with a password

Never place the private key in source files, issues, pull requests, release notes, or chat logs.

## Release flow

The Windows Release workflow:

1. Reads the signing keys from GitHub Actions secrets.
2. Injects the public key and GitHub updater endpoint into the build-only Tauri configuration.
3. Enables `createUpdaterArtifacts`.
4. Builds the Windows NSIS installer.
5. Signs the updater artifact.
6. Publishes the release plus `latest.json` for installed Dusk clients.

Dusk checks:

`https://github.com/bxane-dev/dusk/releases/latest/download/latest.json`

The normal source configuration intentionally does not contain the updater public key because the release workflow injects it during trusted release builds.

## Bootstrap note

Dusk v0.1.0 did not contain the updater plugin. Users of the original v0.1.0 installer must install a newer signed-updater-enabled release manually once. Releases built with the updater signing secrets can then update from inside Dusk.
