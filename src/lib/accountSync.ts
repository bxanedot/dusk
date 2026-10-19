import { api } from "./api";
import { getSupabaseClient } from "./auth";

function hasCloudState(value: unknown) {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  return Array.isArray(record.profiles) && record.profiles.length > 0;
}

let syncInFlight: Promise<void> | null = null;

function cloudSyncEnabled() {
  return localStorage.getItem("dusk-cloud-sync") !== "false";
}

async function syncCloudPlaytime(supabase: NonNullable<ReturnType<typeof getSupabaseClient>>, userId: string) {
  const updates = await api.exportPlaytimeUpdates();
  if (updates.baselines.length) {
    const { error } = await supabase.from("dusk_playtime_baselines").upsert(
      updates.baselines.map((entry) => ({ ...entry, user_id: userId })),
      { onConflict: "user_id,game_id", ignoreDuplicates: true },
    );
    if (error) throw error;
  }
  for (let i = 0; i < updates.sessions.length; i += 200) {
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
  await api.mergeCloudPlaytime(data || []);
}

export function syncAccountState(): Promise<void> {
  if (!cloudSyncEnabled()) return Promise.resolve();
  if (!syncInFlight) {
    syncInFlight = performAccountSync().finally(() => { syncInFlight = null; });
  }
  return syncInFlight;
}

async function performAccountSync() {
  const supabase = getSupabaseClient();
  if (!supabase) throw new Error("Supabase is not configured.");

  const { data: authData, error: authError } = await supabase.auth.getSession();
  if (authError) throw authError;
  const user = authData.session?.user;
  if (!user) throw new Error("Sign in to sync Dusk.");

  await syncCloudPlaytime(supabase, user.id);
  const state = await api.exportAccountState();
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

export async function hydrateAccountState() {
  if (!cloudSyncEnabled()) return false;
  const supabase = getSupabaseClient();
  if (!supabase) return false;

  const { data: authData, error: authError } = await supabase.auth.getSession();
  if (authError) throw authError;
  const user = authData.session?.user;
  if (!user) return false;

  const { data, error } = await supabase
    .from("dusk_account_state")
    .select("state")
    .eq("user_id", user.id)
    .maybeSingle();
  if (error) throw error;

  if (hasCloudState(data?.state)) {
    await api.importAccountState(data!.state as Record<string, unknown>);
    // Merge independent per-device sessions after restoring the legacy account snapshot.
    await syncCloudPlaytime(supabase, user.id);
    return true;
  }

  await syncAccountState();
  return false;
}
