# Release Signing

Dusk can publish a normal Windows installer without private signing material. For the best production experience, configure both updater signing and Windows Authenticode signing as GitHub Actions repository secrets.

## Tauri updater signing

Required for secure in-app updates:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PUBLIC_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the private key uses a password

See `AUTO_UPDATE_SETUP.md` for key generation instructions.

## Windows publisher signing

Required to give the executable a trusted Authenticode publisher identity:

- `WINDOWS_CERTIFICATE` — Base64-encoded PFX certificate
- `WINDOWS_CERTIFICATE_PASSWORD` — PFX password

The release workflow imports the PFX only on the GitHub Actions runner, configures the certificate thumbprint for Tauri, signs with SHA-256, and timestamps the signature.

The certificate private key must never be committed to this repository.

## Release behavior

The release workflow publishes Windows NSIS, Linux `.deb`, and Linux AppImage installers. A configured Windows Authenticode certificate signs the Windows build; if the certificate secrets are absent, the Windows installer remains unsigned.

Updater signing keys are documented for future updater releases. The current release workflow does not publish updater manifests or signatures.
