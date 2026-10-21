import { readFileSync } from 'node:fs';

const SEMVER = '(?:0|[1-9]\\d*)\\.(?:0|[1-9]\\d*)\\.(?:0|[1-9]\\d*)(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?';

export function validateReleaseVersion({ tag, packageVersion, tauriVersion, changelog }) {
  const match = new RegExp(`^dusk-v(${SEMVER})$`).exec(tag ?? '');
  if (!match) throw new Error('Expected a dusk-v semantic-version tag.');

  const version = match[1];
  if (packageVersion !== version || tauriVersion !== version) {
    throw new Error(`Release tag ${version} must match package and Tauri versions (${packageVersion}, ${tauriVersion}).`);
  }

  const lines = changelog.split(/\r?\n/);
  const headingIndex = lines.findIndex((line) => new RegExp(`^##\\s+${version.replaceAll('.', '\\.')}\\s`).test(line));
  if (headingIndex < 0) throw new Error(`CHANGELOG.md is missing a matching changelog section for ${version}.`);

  const section = [];
  for (const line of lines.slice(headingIndex + 1)) {
    if (/^##\s/.test(line)) break;
    section.push(line);
  }
  if (!section.join('\n').trim()) throw new Error(`CHANGELOG.md has an empty release section for ${version}.`);

  return version;
}

function main() {
  const tag = process.argv[2] || process.env.GITHUB_REF_NAME;
  const packageVersion = JSON.parse(readFileSync('package.json', 'utf8')).version;
  const tauriVersion = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
  const changelog = readFileSync('CHANGELOG.md', 'utf8');
  const version = validateReleaseVersion({ tag, packageVersion, tauriVersion, changelog });
  console.log(`Dusk release metadata is consistent for ${version}.`);
}

if (process.argv[1]?.endsWith('release-preflight.mjs')) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
