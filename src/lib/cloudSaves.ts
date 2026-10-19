import type { SupabaseClient } from "@supabase/supabase-js";
import { api } from "./api";
import { getSupabaseClient, SUPABASE_KEY, SUPABASE_URL, supabaseConfigured } from "./auth";
import type { CloudSaveStatus, SaveBackupRecord } from "../types";

let authPromise: Promise<{ client: SupabaseClient; userId: string; accessToken: string }> | null = null;

async function ensureCloudIdentity() {
  if (authPromise) return authPromise;

  authPromise = (async () => {
    const supabase = getSupabaseClient();
    if (!supabase) {
      throw new Error("Supabase cloud saves are not configured for this Dusk build.");
    }

    const existing = await supabase.auth.getSession();
    if (existing.error) throw existing.error;

    const session = existing.data.session;

    if (!session?.user?.id || !session.access_token) {
      throw new Error("Supabase did not return a usable cloud-save session.");
    }

    return {
      client: supabase,
      userId: session.user.id,
      accessToken: session.access_token,
    };
  })();

  try {
    return await authPromise;
  } finally {
    authPromise = null;
  }
}

export async function cloudSaveStatus(): Promise<CloudSaveStatus> {
  if (!supabaseConfigured()) {
    return {
      configured: false,
      authenticated: false,
      userId: null,
      message: "Cloud saves are ready in Dusk, but this build is missing the Supabase project URL/publishable key.",
    };
  }

  try {
    const identity = await ensureCloudIdentity();
    return {
      configured: true,
      authenticated: true,
      userId: identity.userId,
      message: "Dusk account cloud save storage is connected.",
    };
  } catch (error) {
    return {
      configured: true,
      authenticated: false,
      userId: null,
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

export async function syncBackupToSupabase(backup: SaveBackupRecord) {
  const identity = await ensureCloudIdentity();
  return api.uploadSaveBackupToCloud({
    backupId: backup.id,
    supabaseUrl: SUPABASE_URL,
    publishableKey: SUPABASE_KEY,
    accessToken: identity.accessToken,
    authUserId: identity.userId,
  });
}

export async function syncCloudManifest() {
  const identity = await ensureCloudIdentity();
  return api.uploadCloudManifest({
    supabaseUrl: SUPABASE_URL,
    publishableKey: SUPABASE_KEY,
    accessToken: identity.accessToken,
    authUserId: identity.userId,
  });
}

export async function syncAllBackupsToSupabase() {
  const identity = await ensureCloudIdentity();
  return api.uploadAllSaveBackupsToCloud({
    supabaseUrl: SUPABASE_URL,
    publishableKey: SUPABASE_KEY,
    accessToken: identity.accessToken,
    authUserId: identity.userId,
  });
}
