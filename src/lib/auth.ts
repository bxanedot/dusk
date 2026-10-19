import { createClient, type Session, type SupabaseClient, type User } from "@supabase/supabase-js";

const DUSK_SUPABASE_URL = "https://cwfmizfkysdrsesuhkym.supabase.co";
const DUSK_SUPABASE_PUBLISHABLE_KEY = "sb_publishable_yB9lh3iBKpH-tBHFA9neNA_bcNnxbK4";
const DUSK_PASSWORD_RESET_URL =
  "https://cwfmizfkysdrsesuhkym.supabase.co/functions/v1/dusk-password-reset";

export const SUPABASE_URL =
  (import.meta.env.VITE_SUPABASE_URL as string | undefined)?.trim() || DUSK_SUPABASE_URL;
export const SUPABASE_KEY =
  (import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY as string | undefined)?.trim() ||
  DUSK_SUPABASE_PUBLISHABLE_KEY;

let client: SupabaseClient | null = null;

export function supabaseConfigured() {
  return Boolean(SUPABASE_URL && SUPABASE_KEY);
}

export function getSupabaseClient() {
  if (!supabaseConfigured()) return null;
  if (!client) {
    client = createClient(SUPABASE_URL, SUPABASE_KEY, {
      auth: {
        persistSession: true,
        autoRefreshToken: true,
        detectSessionInUrl: false,
        storageKey: "dusk-supabase-auth",
      },
    });
  }
  return client;
}

export type DuskAccount = {
  user: User;
  session: Session;
  username: string;
  displayName: string;
  email: string;
  avatarUrl: string | null;
};

async function loadAvatarUrl(user: User) {
  const supabase = getSupabaseClient();
  if (!supabase) return null;

  // The stable storage object path is persisted in account metadata.
  // Signed URLs are intentionally regenerated whenever the account loads.
  let path =
    typeof user.user_metadata?.avatar_path === "string" &&
    user.user_metadata.avatar_path.trim()
      ? user.user_metadata.avatar_path.trim()
      : "";

  // Backward compatibility for avatars uploaded before avatar_path was saved.
  if (!path) {
    const legacyPath = `${user.id}/avatar`;
    const { data: objects, error: listError } = await supabase.storage
      .from("dusk-avatars")
      .list(user.id, { limit: 20, search: "avatar" });

    if (listError || !objects?.some((item) => item.name === "avatar")) return null;
    path = legacyPath;

    // Best-effort migration so future launches do not need to list the bucket.
    await supabase.auth.updateUser({
      data: { avatar_path: path },
    }).catch(() => undefined);
  }

  const { data, error } = await supabase.storage
    .from("dusk-avatars")
    .createSignedUrl(path, 60 * 60 * 24 * 7);

  if (error) return null;
  return data.signedUrl;
}

async function loadAccountProfile(user: User) {
  const supabase = getSupabaseClient();
  if (!supabase) {
    const fallbackUsername = String(user.user_metadata?.username || "");
    return {
      username: fallbackUsername,
      displayName: String(user.user_metadata?.display_name || fallbackUsername),
    };
  }

  const { data, error } = await supabase
    .from("dusk_accounts")
    .select("username, display_name")
    .eq("user_id", user.id)
    .maybeSingle();

  if (error) throw error;

  const fallbackUsername = String(user.user_metadata?.username || "");
  return {
    username: data?.username || fallbackUsername,
    displayName:
      data?.display_name ||
      String(user.user_metadata?.display_name || data?.username || fallbackUsername),
  };
}

export async function currentDuskAccount(): Promise<DuskAccount | null> {
  const supabase = getSupabaseClient();
  if (!supabase) return null;
  const { data, error } = await supabase.auth.getSession();
  if (error) throw error;
  const session = data.session;
  if (!session?.user) return null;
  const profile = await loadAccountProfile(session.user);
  return {
    user: session.user,
    session,
    username: profile.username,
    displayName: profile.displayName,
    email: session.user.email || "",
    avatarUrl: await loadAvatarUrl(session.user),
  };
}

export async function registerDuskAccount(input: {
  email: string;
  username: string;
  displayName: string;
  password: string;
}): Promise<{ account: DuskAccount | null; message: string }> {
  if (!supabaseConfigured()) {
    throw new Error("Dusk cloud accounts are not configured in this build.");
  }

  const email = input.email.trim();
  const username = input.username.trim();
  const displayName = input.displayName.trim() || username;

  if (!/^[A-Za-z0-9_.-]{3,24}$/.test(username)) {
    throw new Error("Username must be 3-24 characters using letters, numbers, ., _, or -.");
  }
  if (displayName.length < 1 || displayName.length > 48) {
    throw new Error("Display name must be 1-48 characters.");
  }
  if (input.password.length < 6) {
    throw new Error("Password must be at least 6 characters.");
  }

  const response = await fetch(`${SUPABASE_URL}/functions/v1/dusk-register`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      apikey: SUPABASE_KEY,
    },
    body: JSON.stringify({ email, username, displayName, password: input.password }),
  });

  const payload = await response.json().catch(() => ({}));
  if (!response.ok || !payload?.session?.access_token || !payload?.session?.refresh_token) {
    throw new Error(payload?.error || "Could not create Dusk account.");
  }

  const supabase = getSupabaseClient()!;
  const { data, error } = await supabase.auth.setSession({
    access_token: payload.session.access_token,
    refresh_token: payload.session.refresh_token,
  });

  if (error || !data.session || !data.user) {
    throw error || new Error("Dusk could not store the new account session.");
  }

  return {
    account: {
      user: data.user,
      session: data.session,
      username: String(payload.username || username).trim(),
      displayName: String(payload.displayName || displayName).trim(),
      email: data.user.email || email,
      avatarUrl: null,
    },
    message: "Account created and signed in.",
  };
}

export async function loginDuskAccount(username: string, password: string): Promise<DuskAccount> {
  if (!supabaseConfigured()) {
    throw new Error("Dusk cloud accounts are not configured in this build.");
  }
  const response = await fetch(`${SUPABASE_URL}/functions/v1/dusk-username-login`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      apikey: SUPABASE_KEY,
    },
    body: JSON.stringify({ username: username.trim(), password }),
  });

  const payload = await response.json().catch(() => ({}));
  if (!response.ok || !payload?.session?.access_token || !payload?.session?.refresh_token) {
    throw new Error(payload?.error || "Invalid username or password.");
  }

  const supabase = getSupabaseClient()!;
  const { data, error } = await supabase.auth.setSession({
    access_token: payload.session.access_token,
    refresh_token: payload.session.refresh_token,
  });
  if (error || !data.session || !data.user) {
    throw error || new Error("Dusk could not store the account session.");
  }

  const profile = await loadAccountProfile(data.user);
  return {
    user: data.user,
    session: data.session,
    username: profile.username || String(payload.username || username).trim(),
    displayName: profile.displayName,
    email: data.user.email || "",
    avatarUrl: await loadAvatarUrl(data.user),
  };
}

export async function logoutDuskAccount() {
  const supabase = getSupabaseClient();
  if (!supabase) return;
  const { error } = await supabase.auth.signOut();
  if (error) throw error;
}


export async function updateDuskAccount(input: {
  username?: string;
  displayName?: string;
  email?: string;
  currentPassword?: string;
  newPassword?: string;
}): Promise<DuskAccount> {
  const supabase = getSupabaseClient();
  if (!supabase) throw new Error("Supabase is not configured.");

  const { data: sessionData, error: sessionError } = await supabase.auth.getSession();
  if (sessionError) throw sessionError;
  const session = sessionData.session;
  if (!session) throw new Error("Sign in to edit your Dusk account.");

  const response = await fetch(`${SUPABASE_URL}/functions/v1/dusk-update-account`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      apikey: SUPABASE_KEY,
      Authorization: `Bearer ${session.access_token}`,
    },
    body: JSON.stringify(input),
  });

  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload?.error || "Could not update Dusk account.");
  }

  const { data: refreshed } = await supabase.auth.refreshSession();
  const nextSession = refreshed.session || session;
  const fresh = await supabase.auth.getUser();
  const user = fresh.data.user || nextSession.user;
  const profile = await loadAccountProfile(user);

  return {
    user,
    session: nextSession,
    username: profile.username,
    displayName: profile.displayName,
    email: user.email || String(payload.email || input.email || ""),
    avatarUrl: await loadAvatarUrl(user),
  };
}


export async function requestDuskPasswordReset(emailInput: string) {
  const supabase = getSupabaseClient();
  if (!supabase) throw new Error("Dusk cloud accounts are not configured in this build.");

  const email = emailInput.trim().toLowerCase();
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    throw new Error("Enter a valid email address.");
  }

  const { error } = await supabase.auth.resetPasswordForEmail(email, {
    redirectTo: DUSK_PASSWORD_RESET_URL,
  });
  if (error) {
    const message = error.message || "Could not send password reset email.";
    if (/redirect|url/i.test(message)) {
      throw new Error("Dusk password recovery is not fully configured on the account server yet.");
    }
    throw error;
  }

  return "If a Dusk account exists for that email, a password reset link has been sent.";
}


const MAX_DUSK_AVATAR_BYTES = 150 * 1024 * 1024;

export async function uploadDuskAvatar(file: File): Promise<string> {
  const supabase = getSupabaseClient();
  if (!supabase) throw new Error("Supabase is not configured.");

  if (!file.type.startsWith("image/")) {
    throw new Error("Choose an image file for your avatar.");
  }
  if (file.size > MAX_DUSK_AVATAR_BYTES) {
    throw new Error("Avatar images must be 150 MB or smaller.");
  }

  const { data: sessionData, error: sessionError } = await supabase.auth.getSession();
  if (sessionError) throw sessionError;
  const session = sessionData.session;
  if (!session?.user?.id) throw new Error("Sign in to upload an avatar.");

  const path = `${session.user.id}/avatar`;
  const { error: uploadError } = await supabase.storage
    .from("dusk-avatars")
    .upload(path, file, {
      cacheControl: "3600",
      contentType: file.type || "application/octet-stream",
      upsert: true,
    });

  if (uploadError) throw uploadError;

  // Save the stable object path on the Dusk account so the avatar survives
  // restarts and re-logins without relying on bucket listing.
  const { error: metadataError } = await supabase.auth.updateUser({
    data: { avatar_path: path },
  });
  if (metadataError) throw metadataError;

  const { data, error: signedError } = await supabase.storage
    .from("dusk-avatars")
    .createSignedUrl(path, 60 * 60 * 24 * 7);

  if (signedError) throw signedError;
  return data.signedUrl;
}

export async function removeDuskAvatar() {
  const supabase = getSupabaseClient();
  if (!supabase) throw new Error("Supabase is not configured.");

  const { data: sessionData, error: sessionError } = await supabase.auth.getSession();
  if (sessionError) throw sessionError;
  const session = sessionData.session;
  if (!session?.user?.id) throw new Error("Sign in to remove your avatar.");

  const path =
    typeof session.user.user_metadata?.avatar_path === "string" &&
    session.user.user_metadata.avatar_path.trim()
      ? session.user.user_metadata.avatar_path.trim()
      : `${session.user.id}/avatar`;

  const { error } = await supabase.storage
    .from("dusk-avatars")
    .remove([path]);

  if (error) throw error;

  const { error: metadataError } = await supabase.auth.updateUser({
    data: { avatar_path: null },
  });
  if (metadataError) throw metadataError;
}
