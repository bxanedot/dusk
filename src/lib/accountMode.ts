import { withAccountStateLock } from "./accountStateRestore";

let transitionInFlight: Promise<void> | null = null;

export function exitAccountMode(guest: boolean, signOut: () => Promise<void>, reload: () => void): Promise<void> {
  if (!transitionInFlight) {
    transitionInFlight = (async () => {
      const previousMode = localStorage.getItem("dusk-account-mode");
      // Stop cloud work before leaving the account. Keep the native scope
      // until the reloaded account gate selects the next one, so an in-flight
      // sync cannot export guest data under the previous account identity.
      localStorage.setItem("dusk-account-mode", "guest");
      try {
        if (!guest) await signOut();
        // Finish any native import/playtime merge already dispatched before
        // the next page can select a different native account directory.
        await withAccountStateLock(async () => undefined);
      } catch (error) {
        if (previousMode === null) localStorage.removeItem("dusk-account-mode");
        else localStorage.setItem("dusk-account-mode", previousMode);
        throw error;
      }
      localStorage.removeItem("dusk-account-mode");
      reload();
    })().finally(() => { transitionInFlight = null; });
  }
  return transitionInFlight;
}

