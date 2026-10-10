// The session of the dashboard: who signs in, and the routes of the server
// outside the API (`specs/API.md`, `specs/SECURITY.md`).
import type { QueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { meOptions } from "./client/@tanstack/react-query.gen";
import type { ErrorCode, Me } from "./client/types.gen";

const SIGN_IN_PATH = "/auth/github";
const SIGN_OUT_PATH = "/auth/sign-out";
const CSRF_COOKIE = "__Host-cihub_csrf";

/**
 * A path of this site, to open after the sign-in: never another site, as the
 * server checks it too (`page_after` in `crates/web/src/sign_in.rs`).
 */
export const pageOfThisSite = z.string().regex(/^\/(?!\/)[A-Za-z0-9/_.~?=&%-]*$/, "a path of this site");

/** Whether `error` is a problem of the API with the code `code`. */
export function isProblem(error: unknown, code: ErrorCode): boolean {
  return typeof error === "object" && error !== null && "code" in error && error.code === code;
}

/** The user of the session, or `null` without a session. */
export async function readMe(queryClient: QueryClient): Promise<Me | null> {
  try {
    return await queryClient.query(meOptions());
  } catch (error) {
    if (isProblem(error, "unauthorized")) {
      return null;
    }
    throw error;
  }
}

/** The address that starts a sign-in, then opens `next`. */
export function signInAddress(next: string, chooseAccount = false): string {
  const query = new URLSearchParams({ next });
  if (chooseAccount) {
    query.set("choose_account", "true");
  }
  return `${SIGN_IN_PATH}?${query.toString()}`;
}

/**
 * Ends the session; the CSRF token comes from its cookie. Whether the server
 * ended it: without that, the old session stays valid.
 */
export async function signOut(): Promise<boolean> {
  const csrf = document.cookie
    .split("; ")
    .find((pair) => pair.startsWith(`${CSRF_COOKIE}=`))
    ?.slice(CSRF_COOKIE.length + 1);
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
