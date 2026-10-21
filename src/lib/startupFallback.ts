export type StartupAccountProfile = {
  username: string;
  displayName: string;
};

export function fallbackAccountProfile(user: {
  email?: string | null;
  user_metadata?: Record<string, unknown> | null;
}): StartupAccountProfile {
  const emailName = user.email?.split("@", 1)[0] || "";
  const username = String(user.user_metadata?.username || emailName);
  return {
    username,
    displayName: String(user.user_metadata?.display_name || username),
  };
}

export function withStartupFallback<T>(request: Promise<T>, fallback: T, timeoutMs: number): Promise<T> {
  return new Promise(resolve => {
    const timeout = setTimeout(() => resolve(fallback), timeoutMs);
    request.then(
      value => {
        clearTimeout(timeout);
        resolve(value);
      },
      () => {
        clearTimeout(timeout);
        resolve(fallback);
      },
    );
  });
}
