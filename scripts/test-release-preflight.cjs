const assert = require('node:assert/strict');
const { test } = require('node:test');

test('accepts a release tag when app, Tauri, and changelog versions agree', async () => {
  const { validateReleaseVersion } = await import('./release-preflight.mjs');

  assert.equal(
    validateReleaseVersion({
      tag: 'dusk-v1.0.0',
      packageVersion: '1.0.0',
      tauriVersion: '1.0.0',
      changelog: '# Changelog\n\n## 1.0.0 - 2026-10-19\n\nStable release notes.\n',
    }),
    '1.0.0'
  );
});

test('rejects a release tag that does not match app versions', async () => {
  const { validateReleaseVersion } = await import('./release-preflight.mjs');

  assert.throws(
    () => validateReleaseVersion({
      tag: 'dusk-v1.1.0',
      packageVersion: '1.0.0',
      tauriVersion: '1.0.0',
      changelog: '# Changelog\n\n## 1.1.0 - 2026-10-21\n\nRelease notes.\n',
    }),
    /must match package and Tauri versions/
  );
});

test('rejects a release tag without a matching changelog section', async () => {
  const { validateReleaseVersion } = await import('./release-preflight.mjs');

  assert.throws(
    () => validateReleaseVersion({
      tag: 'dusk-v1.0.0',
      packageVersion: '1.0.0',
      tauriVersion: '1.0.0',
      changelog: '# Changelog\n\n## [Unreleased]\n\nDraft notes.\n',
    }),
    /matching changelog section/
  );
});

test('rejects malformed Dusk tags', async () => {
  const { validateReleaseVersion } = await import('./release-preflight.mjs');

  assert.throws(
    () => validateReleaseVersion({
      tag: 'v1.0.0',
      packageVersion: '1.0.0',
      tauriVersion: '1.0.0',
      changelog: '# Changelog\n\n## 1.0.0 - 2026-10-19\n\nStable release notes.\n',
    }),
    /Expected a dusk-v semantic-version tag/
  );
});
