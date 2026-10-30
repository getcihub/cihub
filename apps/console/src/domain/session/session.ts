// The session of the dashboard: who signs in, and the routes of the server
// outside the API (`specs/API.md`, `specs/SECURITY.md`).
import { client } from "@/client/client.gen";

const SIGN_IN_PATH = "/auth/github";
const SIGN_OUT_PATH = "/auth/sign-out";
const CSRF_COOKIE = "__Host-cihub_csrf";

/** The CSRF token of the session, from its cookie; a script only reads it. */
function csrfToken(): string | undefined {
  return document.cookie
    .split("; ")
    .find((pair) => pair.startsWith(`${CSRF_COOKIE}=`))
    ?.slice(CSRF_COOKIE.length + 1);
}

/** Sends the CSRF token of the session with each request of the API that can change a state. */
export function sendCsrfToken(): void {
  client.interceptors.request.use((request) => {
    const token = csrfToken();
    if (token && !["GET", "HEAD", "OPTIONS"].includes(request.method)) {
      request.headers.set("X-CSRF-Token", token);
    }
    return request;
  });
}

/** The address that starts a sign-in, then opens `next`. */
export function signInAddress(next: string): string {
  return `${SIGN_IN_PATH}?${new URLSearchParams({ next }).toString()}`;
}

/**
 * Ends the session; the CSRF token comes from its cookie. Whether the server
 * ended it: without that, the old session stays valid.
 */
export async function signOut(): Promise<boolean> {
  const csrf = csrfToken();
  try {
    const response = await fetch(SIGN_OUT_PATH, {
      method: "POST",
      headers: csrf ? { "X-CSRF-Token": csrf } : {},
    });
    return response.ok;
  } catch {
    return false;
  }
}
