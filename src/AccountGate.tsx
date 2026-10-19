import { type FormEvent, type MouseEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { ArrowLeft, KeyRound, LockKeyhole, LogIn, Mail, Minus, Square, UserRound, UserPlus, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import duskLogo from "./assets/dusk-logo.svg";
import {
  currentDuskAccount,
  loginDuskAccount,
  registerDuskAccount,
  requestDuskPasswordReset,
  supabaseConfigured,
  type DuskAccount,
} from "./lib/auth";
import { hydrateAccountState } from "./lib/accountSync";
import { api } from "./lib/api";

type Mode = "login" | "register" | "forgot";

function AccountWindowBar() {
  async function handleMouseDown(event: MouseEvent<HTMLElement>) {
    if (event.button !== 0) return;
    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button")) return;

    const appWindow = getCurrentWindow();
    if (event.detail === 2) {
      await appWindow.toggleMaximize().catch(() => undefined);
      return;
    }
    await appWindow.startDragging().catch(() => undefined);
  }

  return (
    <header
      className="account-window-titlebar"
      data-tauri-drag-region
      onMouseDown={(event) => void handleMouseDown(event)}
    >
      <div className="account-window-brand" data-tauri-drag-region>
        <img src={duskLogo} alt="" draggable={false} />
        <span>Dusk</span>
      </div>
      <div className="account-window-controls">
        <button
          type="button"
          className="account-window-control"
          aria-label="Minimize Dusk"
          title="Minimize"
          onClick={() => void getCurrentWindow().minimize()}
        >
          <Minus size={15} />
        </button>
        <button
          type="button"
          className="account-window-control"
          aria-label="Maximize or restore Dusk"
          title="Maximize / restore"
          onClick={() => void getCurrentWindow().toggleMaximize()}
        >
          <Square size={12} />
        </button>
        <button
          type="button"
          className="account-window-control close"
          aria-label="Close Dusk"
          title="Close"
          onClick={() => void getCurrentWindow().close()}
        >
          <X size={15} />
        </button>
      </div>
    </header>
  );
}

export default function AccountGate(props: { children: ReactNode }) {
  const [account, setAccount] = useState<DuskAccount | null>(null);
  const [guest, setGuest] = useState(false);
  const [checking, setChecking] = useState(true);
  const [mode, setMode] = useState<Mode>("login");
  const [email, setEmail] = useState("");
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const guestChoiceRef = useRef(false);
  const accountScopeInitializationRef = useRef<Promise<void> | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        if (localStorage.getItem("dusk-account-mode") === "guest") {
          // A fresh process starts in the unscoped local-library mode. Clear
          // any stale scope asynchronously so a slow native command cannot
          // keep the account gate over the whole application.
          void api.setAccountScope(null).catch((error: unknown) => {
            console.warn("Could not initialize guest account scope:", error);
          });
          if (!guestChoiceRef.current) setGuest(true);
          return;
        }

        // A network timeout during session refresh must not hold the whole UI
        // on the noninteractive "Opening Dusk" screen indefinitely.
        let sessionTimer: ReturnType<typeof setTimeout> | undefined;
        const session = currentDuskAccount();
        const timeout = new Promise<null>(resolve => {
          sessionTimer = setTimeout(() => resolve(null), 10_000);
        });
        const existing = await Promise.race([session, timeout]).finally(() => {
          if (sessionTimer !== undefined) clearTimeout(sessionTimer);
        });
        if (guestChoiceRef.current) return;
        if (existing) {
          const scopeInitialization = api.setAccountScope(existing.user.id);
          accountScopeInitializationRef.current = scopeInitialization;
          await scopeInitialization;
          if (guestChoiceRef.current) return;
          // Cloud hydration can be slow or never resolve on offline/filtered
          // networks. Do not make the entire main application unclickable
          // while waiting for an external service.
          setAccount(existing);
          void hydrateAccountState().catch((error: unknown) => {
            console.warn("Dusk cloud account hydration unavailable:", error);
          });
        } else {
          await api.setAccountScope(null);
        }
      } catch (error) {
        if (!guestChoiceRef.current) setMessage(error instanceof Error ? error.message : String(error));
      } finally {
        if (!guestChoiceRef.current) setChecking(false);
      }
    })();
  }, []);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setMessage("");

    try {
      if (mode === "forgot") {
        const result = await requestDuskPasswordReset(email);
        setMessage(result);
      } else if (mode === "register") {
        const result = await registerDuskAccount({ email, username, displayName, password });
        setMessage(result.message);
        if (result.account) {
          localStorage.removeItem("dusk-account-mode");
          await api.setAccountScope(result.account.user.id);
          await hydrateAccountState();
          setAccount(result.account);
        }
      } else {
        const signedIn = await loginDuskAccount(username, password);
        localStorage.removeItem("dusk-account-mode");
        await api.setAccountScope(signedIn.user.id);
        await hydrateAccountState();
        setAccount(signedIn);
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function continueAsGuest() {
    guestChoiceRef.current = true;
    setBusy(true);
    setMessage("");
    try {
      localStorage.setItem("dusk-account-mode", "guest");
      // If a restored account was already selecting its local data directory,
      // finish that transition first so the guest scope is always the last
      // scope applied before mounting the library.
      await accountScopeInitializationRef.current?.catch(() => undefined);
      await api.setAccountScope(null);
      setGuest(true);
      setChecking(false);
    } catch (error) {
      guestChoiceRef.current = false;
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  if (checking) {
    return (
      <main className="account-shell">
        <AccountWindowBar />
        <div className="account-card compact">
          <div className="account-logo">
            <img className="account-logo-image" src={duskLogo} alt="" draggable={false} />
          </div>
          <strong>Opening Dusk...</strong>
          <p className="account-guest-note">Account checks can be slow when offline. You can open your local library without signing in.</p>
          <button className="account-guest" type="button" disabled={busy} onClick={() => void continueAsGuest()}>
            <UserRound size={17} /> {busy ? "Opening local library…" : "Continue as guest"}
          </button>
        </div>
      </main>
    );
  }

  if (account || guest) return <>{props.children}</>;

  return (
    <main className="account-shell">
      <AccountWindowBar />
      <section className="account-card">
        <div className="account-brand">
          <div className="account-logo">
            <img className="account-logo-image" src={duskLogo} alt="Dusk" draggable={false} />
          </div>
          <div>
            <span>DUSK ACCOUNT</span>
            <h1>
              {mode === "login"
                ? "Welcome back"
                : mode === "register"
                  ? "Create your account"
                  : "Reset your password"}
            </h1>
            <p>
              {mode === "forgot"
                ? "Enter the email on your Dusk account. We’ll send a secure password reset link."
                : "Sync your Dusk library, profiles, progress, collections, and private cloud saves across devices."}
            </p>
          </div>
        </div>

        {!supabaseConfigured() && (
          <div className="account-message error">
            Dusk cloud accounts are temporarily unavailable. Guest mode still works.
          </div>
        )}

        <form className="account-form" onSubmit={submit}>
          {mode === "forgot" ? (
            <>
              <label>
                <span>Email</span>
                <div className="account-field-icon">
                  <Mail size={15} />
                  <input
                    type="email"
                    autoComplete="email"
                    value={email}
                    onChange={(event) => setEmail(event.target.value)}
                    placeholder="you@example.com"
                    required
                  />
                </div>
              </label>
              <div className="account-recovery-note">
                <KeyRound size={15} />
                <span>The email link opens Dusk’s hosted reset page where you can choose a new password.</span>
              </div>
            </>
          ) : (
            <>
          {mode === "register" && (
            <>
              <label>
                <span>Display name</span>
                <input
                  autoComplete="name"
                  value={displayName}
                  onChange={(event) => setDisplayName(event.target.value)}
                  placeholder="Your name"
                  maxLength={48}
                  required
                />
              </label>
              <label>
                <span>Email</span>
                <input
                  type="email"
                  autoComplete="email"
                  value={email}
                  onChange={(event) => setEmail(event.target.value)}
                  placeholder="you@example.com"
                  required
                />
              </label>
            </>
          )}
          <label>
            <span>Username</span>
            <input
              autoComplete="username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              placeholder="username"
              minLength={3}
              maxLength={24}
              required
            />
          </label>
          <label>
            <span>Password</span>
            <input
              type="password"
              autoComplete={mode === "login" ? "current-password" : "new-password"}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="********"
              minLength={6}
              required
            />
          </label>

            </>
          )}

          {message && <div className="account-message">{message}</div>}

          <button className="account-submit" disabled={busy || !supabaseConfigured()} type="submit">
            {mode === "forgot" ? (
              <Mail size={17} />
            ) : mode === "login" ? (
              <LogIn size={17} />
            ) : (
              <UserPlus size={17} />
            )}
            {busy
              ? "Working..."
              : mode === "forgot"
                ? "Send reset link"
                : mode === "login"
                  ? "Sign in"
                  : "Create account"}
          </button>
        </form>

        {mode === "login" && (
          <button
            className="account-forgot"
            type="button"
            disabled={busy}
            onClick={() => {
              setMode("forgot");
              setEmail("");
              setPassword("");
              setMessage("");
            }}
          >
            <KeyRound size={15} />
            Forgot password?
          </button>
        )}

        <button
          className="account-switch"
          type="button"
          onClick={() => {
            setMode(mode === "login" ? "register" : "login");
            setMessage("");
            setPassword("");
          }}
        >
          {mode === "forgot" ? <ArrowLeft size={15} /> : <LockKeyhole size={15} />}
          {mode === "login"
            ? "Need an account? Register"
            : mode === "register"
              ? "Already have an account? Sign in"
              : "Back to sign in"}
        </button>

        <div className="account-divider"><span>or</span></div>

        <button
          className="account-guest"
          type="button"
          disabled={busy}
          onClick={() => void continueAsGuest()}
        >
          <UserRound size={17} />
          Continue as guest
        </button>
        <p className="account-guest-note">
          Guest mode keeps your library and saves local to this PC. You can sign in later from Settings.
        </p>
      </section>
    </main>
  );
}
