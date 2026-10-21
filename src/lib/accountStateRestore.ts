type Snapshot = Record<string, unknown>;
type Row = Record<string, unknown>;

let stateQueue: Promise<void> = Promise.resolve();

// Keep the final local read and snapshot import together. Navigation and reads
// remain available; mutations wait only for other native mutations/imports.
export function withAccountStateLock<T>(operation: () => Promise<T>): Promise<T> {
  const result = stateQueue.then(operation);
  stateQueue = result.then(() => undefined, () => undefined);
  return result;
}

function rows(state: Snapshot, field: string): Row[] {
  const value = state[field];
  return Array.isArray(value)
    ? value.filter((row): row is Row => !!row && typeof row === "object" && !Array.isArray(row))
    : [];
}

const same = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);
const keyOf = (row: Row, fields: string[]) => JSON.stringify(fields.map(field => row[field]));

// The cloud snapshot is authoritative for unchanged fields. Edits/removals made
// since the download began take precedence, including after a failed read retry.
export function preservePendingAccountEdits(cloud: Snapshot, baseline: Snapshot, local: Snapshot): Snapshot {
  const merged = { ...cloud };
  const deleted = new Map<string, Set<unknown>>();
  for (const field of ["profiles", "games", "collections"]) {
    const currentIds = new Set(rows(local, field).map(row => row.id));
    deleted.set(field, new Set(rows(baseline, field).map(row => row.id).filter(id => !currentIds.has(id))));
  }

  const tables: Array<[string, string[]]> = [
    ["profiles", ["id"]],
    ["games", ["id"]],
    ["collections", ["id"]],
    ["profileGames", ["profileId", "gameId"]],
    ["collectionGames", ["collectionId", "gameId"]],
    ["platformAchievements", ["provider", "steamId", "gameId", "achievementId"]],
  ];
  for (const [field, keys] of tables) {
    const before = new Map(rows(baseline, field).map(row => [keyOf(row, keys), row]));
    const current = new Map(rows(local, field).map(row => [keyOf(row, keys), row]));
    const result: Row[] = [];
    const seen = new Set<string>();
    for (const remote of rows(cloud, field)) {
      const key = keyOf(remote, keys);
      seen.add(key);
      const original = before.get(key);
      const edited = current.get(key);
      if (original && !edited) continue;
      if (deleted.get("profiles")?.has(remote.profileId)
        || deleted.get("games")?.has(remote.gameId)
        || deleted.get("collections")?.has(remote.collectionId)) continue;
      const row = { ...remote };
      if (edited) {
        for (const name of Object.keys(edited)) {
          if (!original || !same(original[name], edited[name])) row[name] = edited[name];
        }
        // Legacy totals are cumulative. Preserve the native import's MAX
        // semantics even when this device accumulated time during the read.
        if (field === "games") {
          for (const name of ["totalSeconds", "launchCount"]) {
            const values = [remote[name], edited[name]].filter((value): value is number =>
              typeof value === "number" && Number.isFinite(value));
            if (values.length) row[name] = Math.max(...values);
          }
        }
      }
      result.push(row);
    }
    for (const [key, row] of current) {
      if (!seen.has(key)) result.push(row);
    }
    merged[field] = result;
  }
  if (!same(baseline.activeProfileId, local.activeProfileId)) {
    merged.activeProfileId = local.activeProfileId;
  }
  return merged;
}
