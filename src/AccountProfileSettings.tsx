import { type ChangeEvent, type FormEvent, useEffect, useRef, useState } from "react";
import { Camera, KeyRound, Mail, Save, ShieldCheck, Trash2, UserRound } from "lucide-react";
import {
  currentDuskAccount,
  removeDuskAvatar,
  updateDuskAccount,
  uploadDuskAvatar,
  type DuskAccount,
} from "./lib/auth";

export default function AccountProfileSettings(props: {
  onToast: (message: string, type?: "ok" | "error") => void;
}) {
  const [account, setAccount] = useState<DuskAccount | null>(null);
  const [loading, setLoading] = useState(true);
  const [profileBusy, setProfileBusy] = useState(false);
  const [securityBusy, setSecurityBusy] = useState(false);
  const [avatarBusy, setAvatarBusy] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement | null>(null);

  const [displayName, setDisplayName] = useState("");
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  useEffect(() => {
    let cancelled = false;
    void currentDuskAccount()
      .then((value) => {
        if (cancelled || !value) return;
        setAccount(value);
        setDisplayName(value.displayName);
        setUsername(value.username);
        setEmail(value.email);
      })
      .catch((error) => {
        if (!cancelled) {
          props.onToast(error instanceof Error ? error.message : String(error), "error");
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [props.onToast]);

  async function changeAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file || avatarBusy || !account) return;

    setAvatarBusy(true);
    try {
      const avatarUrl = await uploadDuskAvatar(file);
      const next = { ...account, avatarUrl };
      setAccount(next);
      window.dispatchEvent(new CustomEvent("dusk-account-avatar-changed", { detail: next }));
      props.onToast("Profile avatar updated.");
    } catch (error) {
      props.onToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setAvatarBusy(false);
    }
  }

  async function removeAvatar() {
    if (avatarBusy || !account?.avatarUrl) return;
    setAvatarBusy(true);
    try {
      await removeDuskAvatar();
      const next = { ...account, avatarUrl: null };
      setAccount(next);
      window.dispatchEvent(new CustomEvent("dusk-account-avatar-changed", { detail: next }));
      props.onToast("Profile avatar removed.");
    } catch (error) {
      props.onToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setAvatarBusy(false);
    }
  }

  async function saveProfile(event: FormEvent) {
    event.preventDefault();
    if (profileBusy) return;

    const cleanDisplayName = displayName.trim();
    const cleanUsername = username.trim();
    if (!cleanDisplayName) {
      props.onToast("Display name cannot be empty.", "error");
      return;
    }

    setProfileBusy(true);
    try {
      const updated = await updateDuskAccount({
        displayName: cleanDisplayName,
        username: cleanUsername,
      });
      setAccount(updated);
      setDisplayName(updated.displayName);
      setUsername(updated.username);
      setEmail(updated.email);
      window.dispatchEvent(new CustomEvent("dusk-account-avatar-changed", { detail: updated }));
      props.onToast("Dusk account profile updated.");
    } catch (error) {
      props.onToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setProfileBusy(false);
    }
  }

  async function saveSecurity(event: FormEvent) {
    event.preventDefault();
    if (securityBusy || !account) return;

    const cleanEmail = email.trim().toLowerCase();
    const changingEmail = cleanEmail !== account.email.toLowerCase();
    const changingPassword = Boolean(newPassword);

    if (!changingEmail && !changingPassword) {
      props.onToast("No email or password changes to save.");
      return;
    }
    if (!currentPassword) {
      props.onToast("Enter your current password first.", "error");
      return;
    }
    if (changingPassword && newPassword !== confirmPassword) {
      props.onToast("New passwords do not match.", "error");
      return;
    }

    setSecurityBusy(true);
    try {
      const updated = await updateDuskAccount({
        ...(changingEmail ? { email: cleanEmail } : {}),
        currentPassword,
        ...(changingPassword ? { newPassword } : {}),
      });
      setAccount(updated);
      setEmail(updated.email);
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      props.onToast(
        changingEmail && changingPassword
          ? "Email and password updated."
          : changingEmail
            ? "Email updated."
            : "Password updated.",
      );
    } catch (error) {
      props.onToast(error instanceof Error ? error.message : String(error), "error");
    } finally {
      setSecurityBusy(false);
    }
  }

  if (loading) {
    return (
      <details className="settings-card account-profile-card">
        <summary className="settings-card-head">
          <UserRound size={20} />
          <div>
            <h3>Dusk account profile</h3>
            <p>Loading your account details…</p>
          </div>
        </summary>
      </details>
    );
  }

  if (!account) return null;

  return (
    <details className="settings-card account-profile-card">
      <summary className="settings-card-head">
        <UserRound size={20} />
        <div>
          <h3>Dusk account profile</h3>
          <p>Edit the identity attached to your synced Dusk account.</p>
        </div>
      </summary>

      <form className="account-settings-form" onSubmit={saveProfile}>
        <div className="account-settings-section-head">
          <UserRound size={16} />
          <div>
            <strong>Profile</strong>
            <span>Shown inside Dusk. Username is also used to sign in.</span>
          </div>
        </div>

        <div className="account-avatar-editor">
          <div className="account-avatar-preview" aria-label="Profile avatar preview">
            {account.avatarUrl ? (
              <img src={account.avatarUrl} alt={displayName + " avatar"} />
            ) : (
              <span>{(displayName || username || "D").trim().charAt(0).toUpperCase()}</span>
            )}
          </div>

          <div className="account-avatar-copy">
            <strong>Profile image</strong>
            <span>Upload any image up to 150 MB. Dusk crops the square image into a round avatar.</span>
            <div className="account-avatar-actions">
              <input
                ref={avatarInputRef}
                className="account-avatar-file"
                type="file"
                accept="image/*"
                onChange={(event) => void changeAvatar(event)}
              />
              <button
                className="button secondary"
                type="button"
                disabled={avatarBusy}
                onClick={() => avatarInputRef.current?.click()}
              >
                <Camera size={15} />
                {avatarBusy ? "Uploading…" : account.avatarUrl ? "Change avatar" : "Upload avatar"}
              </button>
              {account.avatarUrl && (
                <button
                  className="button secondary danger"
                  type="button"
                  disabled={avatarBusy}
                  onClick={() => void removeAvatar()}
                >
                  <Trash2 size={15} />
                  Remove
                </button>
              )}
            </div>
          </div>
        </div>

        <div className="account-settings-grid">
          <label>
            <span>Display name</span>
            <input
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
              maxLength={48}
              autoComplete="name"
              placeholder="Your name"
              required
            />
          </label>
          <label>
            <span>Username</span>
            <input
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              minLength={3}
              maxLength={24}
              autoComplete="username"
              placeholder="username"
              required
            />
          </label>
        </div>

        <div className="account-settings-actions">
          <span className="account-settings-identity">@{account.username}</span>
          <button className="button primary" type="submit" disabled={profileBusy}>
            <Save size={15} />
            {profileBusy ? "Saving…" : "Save profile"}
          </button>
        </div>
      </form>

      <div className="account-settings-divider" />

      <form className="account-settings-form" onSubmit={saveSecurity}>
        <div className="account-settings-section-head">
          <ShieldCheck size={16} />
          <div>
            <strong>Security</strong>
            <span>Email and password changes require your current password.</span>
          </div>
        </div>

        <div className="account-settings-grid">
          <label className="account-settings-wide">
            <span>Email</span>
            <div className="account-input-with-icon">
              <Mail size={15} />
              <input
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                autoComplete="email"
                placeholder="you@example.com"
                required
              />
            </div>
          </label>
          <label>
            <span>Current password</span>
            <div className="account-input-with-icon">
              <KeyRound size={15} />
              <input
                type="password"
                value={currentPassword}
                onChange={(event) => setCurrentPassword(event.target.value)}
                autoComplete="current-password"
                placeholder="Required for security changes"
              />
            </div>
          </label>
          <label>
            <span>New password</span>
            <input
              type="password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
              autoComplete="new-password"
              minLength={8}
              placeholder="Leave blank to keep current"
            />
          </label>
          <label>
            <span>Confirm new password</span>
            <input
              type="password"
              value={confirmPassword}
              onChange={(event) => setConfirmPassword(event.target.value)}
              autoComplete="new-password"
              minLength={8}
              placeholder="Repeat new password"
            />
          </label>
        </div>

        <div className="account-settings-actions">
          <span className="account-settings-identity">{account.email}</span>
          <button className="button primary" type="submit" disabled={securityBusy}>
            <ShieldCheck size={15} />
            {securityBusy ? "Updating…" : "Update security"}
          </button>
        </div>
      </form>
    </details>
  );
}
