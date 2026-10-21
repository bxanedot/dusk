import { api } from "./api";
import { getSupabaseClient } from "./auth";
import { preservePendingAccountEdits, withAccountStateLock } from "./accountStateRestore";

function hasCloudState(value: unknown) {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  return Array.isArray(record.profiles) && record.profiles.length > 0;
}

let syncInFlight: Promise<void> | null = null;
let hydrationInFlight: Promise<boolean> | null = null;
let hydrationRequired = true;
let hydrationBaseline: { userId: string | null; state: Record<string, unknown> } | null = null;

export const ACCOUNT_STATE_HYDRATED_EVENT = "dusk-account-state-hydrated";

function cloudSyncEnabled() {
  return localStorage.getItem("dusk-account-mode") !== "guest"
    && localStorage.getItem("dusk-cloud-sync") !== "false";
}

async function syncCloudPlaytime(supabase: NonNullable<ReturnType<typeof getSupabaseClient>>, userId: string) {
  if (!cloudSyncEnabled()) return;
  const updates = await api.exportPlaytimeUpdates();
  if (!cloudSyncEnabled()) return;
  if (updates.baselines.length) {
    const { error } = await supabase.from("dusk_playtime_baselines").upsert(
      updates.baselines.map((entry) => ({ ...entry, user_id: userId })),
      { onConflict: "user_id,game_id", ignoreDuplicates: true },
    );
    if (error) throw error;
  }
  for (let i = 0; i < updates.sessions.length; i += 200) {
    if (!cloudSyncEnabled()) return;
    const batch = updates.sessions.slice(i, i + 200)
      .filter((entry) => entry.duration_seconds > 0);
    if (!batch.length) continue;
    const { error } = await supabase.from("dusk_playtime_sessions").upsert(
      batch.map((entry) => ({ ...entry, user_id: userId })),
      { onConflict: "user_id,device_id,session_id", ignoreDuplicates: true },
    );
    if (error) throw error;
  }
  const { data, error } = await supabase.rpc("dusk_playtime_totals");
  if (error) throw error;
  if (!cloudSyncEnabled()) return;
  await api.mergeCloudPlaytime(data || []);
}

export function syncAccountState(): Promise<void> {
  if (!cloudSyncEnabled()) return Promise.resolve();
  if (!syncInFlight) {
    syncInFlight = (async () => {
      // The first local refresh can finish before the cloud snapshot arrives.
      // Restore it first so an empty local library cannot replace remote data.
      // Hydration itself initializes accounts that have no remote snapshot.
      if (hydrationInFlight || hydrationRequired) {
        await (hydrationInFlight ?? hydrateAccountState());
        return;
      }
      await performAccountSync();
    })().finally(() => { syncInFlight = null; });
  }
  return syncInFlight;
}

async function performAccountSync() {
  if (!cloudSyncEnabled()) return;
  const supabase = getSupabaseClient();
  if (!supabase) throw new Error("Supabase is not configured.");

  const { data: authData, error: authError } = await supabase.auth.getSession();
  if (authError) throw authError;
  const user = authData.session?.user;
  if (!user) throw new Error("Sign in to sync Dusk.");

  await syncCloudPlaytime(supabase, user.id);
  if (!cloudSyncEnabled()) return;
  const state = await api.exportAccountState();
  if (!cloudSyncEnabled()) return;
  const { error } = await supabase.from("dusk_account_state").upsert(
    {
      user_id: user.id,
      state,
      state_version: 1,
      updated_at: new Date().toISOString(),
    },
    { onConflict: "user_id" },
  );
  if (error) throw error;
}

export function hydrateAccountState(): Promise<boolean> {
  if (!cloudSyncEnabled()) return Promise.resolve(false);
  if (!hydrationInFlight) {
    hydrationRequired = true;
    hydrationInFlight = performAccountHydration()
      .then(restored => {
        if (cloudSyncEnabled()) hydrationRequired = false;
        return restored;
      })
      .finally(() => { hydrationInFlight = null; });
  }
  return hydrationInFlight;
}

async function performAccountHydration() {
  if (!cloudSyncEnabled()) return false;
  const supabase = getSupabaseClient();
  if (!supabase) return false;

  // The local library is already usable while authentication refreshes. Capture
  // its baseline first and retain it if authentication fails before identifying
  // the account, so a retry cannot mistake intervening edits for unchanged data.
  const initialState = await withAccountStateLock(() => api.exportAccountState());
  if (!cloudSyncEnabled()) return false;
  hydrationBaseline ??= { userId: null, state: initialState };

  const { data: authData, error: authError } = await supabase.auth.getSession();
  if (authError) throw authError;
  const user = authData.session?.user;
  if (!user) return false;

  if (hydrationBaseline.userId !== null && hydrationBaseline.userId !== user.id) {
    hydrationBaseline = { userId: user.id, state: initialState };
  } else {
    hydrationBaseline.userId = user.id;
  }
  const baseline = hydrationBaseline.state;

  const { data, error } = await supabase
    .from("dusk_account_state")
    .select("state")
    .eq("user_id", user.id)
    .maybeSingle();
  if (error) throw error;
  if (!cloudSyncEnabled()) return false;

  if (hasCloudState(data?.state)) {
    const imported = await withAccountStateLock(async () => {
      if (!cloudSyncEnabled()) return false;
      const local = await api.exportAccountState();
      if (!cloudSyncEnabled()) return false;
      const state = preservePendingAccountEdits(data!.state as Record<string, unknown>, baseline, local);
      await api.importAccountState(state);
      return true;
    });
    if (!imported) return false;
    // Playtime failures must not cause a later sync to repeat this import.
    hydrationRequired = false;
    hydrationBaseline = null;
    window.dispatchEvent(new Event(ACCOUNT_STATE_HYDRATED_EVENT));
    // Merge independent per-device sessions after restoring the legacy account snapshot.
    await syncCloudPlaytime(supabase, user.id);
    if (cloudSyncEnabled()) window.dispatchEvent(new Event(ACCOUNT_STATE_HYDRATED_EVENT));
    return true;
  }

  // Use the upload operation directly; public sync waits for this hydration.
  await performAccountSync();
  hydrationBaseline = null;
  return false;
}
