// The session of the dashboard: who signs in, and the routes of the server
// outside the API (`specs/API.md`, `specs/SECURITY.md`).
import { type QueryClient, useSuspenseQueries } from "@tanstack/react-query";
import { z } from "zod";

import {
  getInstanceOptions,
  getUserOptions,
  getUserQueryKey,
  listUserOrgsOptions,
  listUserOrgsQueryKey,
} from "./client/@tanstack/react-query.gen";
import { client } from "./client/client.gen";
import type { ErrorCode, Org } from "./client/types.gen";

const SIGN_IN_PATH = "/auth/github";
const SIGN_OUT_PATH = "/auth/sign-out";
const CSRF_COOKIE = "__Host-cihub_csrf";

/**
 * A path of this site, to open after the sign-in: never another site, as the
 * server checks it too (`page_after` in `crates/web/src/sign_in.rs`).
 */
export const pageOfThisSite = z.string().regex(/^\/(?!\/)[A-Za-z0-9/_.~?=&%-]*$/, "a path of this site");

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

/** Whether `error` is a problem of the API with the code `code`. */
export function isProblem(error: unknown, code: ErrorCode): boolean {
  return typeof error === "object" && error !== null && "code" in error && error.code === code;
}

/** The person of the session, their organizations, and the instance: `/user`, `/user/orgs` and `/instance`. */
export interface Me {
  login: string;
  orgs: Org[];
  version: string;
  app_install_url?: string | null;
}

/** A person has a few organizations: one page holds them all. */
const ORGS_OF_THE_USER = { query: { limit: 100 } };

const readsOfMe = () =>
  [getUserOptions(), listUserOrgsOptions(ORGS_OF_THE_USER), getInstanceOptions()] as const;

/** The person of the session, or `null` without a session. */
export async function readMe(queryClient: QueryClient): Promise<Me | null> {
  const [user, orgs, instance] = readsOfMe();
  const [readUser, readOrgs, readInstance] = await Promise.allSettled([
    queryClient.query(user),
    queryClient.query(orgs),
    queryClient.query(instance),
  ]);
  const failures = [readUser, readOrgs, readInstance].flatMap((read) =>
    read.status === "rejected" ? [read.reason as unknown] : [],
  );

  if (failures.some((error) => isProblem(error, "unauthorized"))) {
    return null;
  }
  if (
    readUser.status === "rejected" ||
    readOrgs.status === "rejected" ||
    readInstance.status === "rejected"
  ) {
    throw failures[0];
  }
  return meOf(readUser.value, readOrgs.value, readInstance.value);
}

/** The person of the session, from the reads of `readMe` in the cache. */
export function useMe(): Me {
  const [user, orgs, instance] = useSuspenseQueries({ queries: readsOfMe() });
  return meOf(user.data, orgs.data, instance.data);
}

function meOf(
  user: { login: string },
  orgs: { orgs: Org[] },
  instance: { version: string; app_install_url?: string | null },
): Me {
  return {
    login: user.login,
    orgs: orgs.orgs,
    version: instance.version,
    app_install_url: instance.app_install_url,
  };
}

/** Forgets the person of the session: a read still fresh in the cache would send them back from the sign-in. */
export function forgetMe(queryClient: QueryClient): void {
  queryClient.removeQueries({ queryKey: getUserQueryKey() });
  queryClient.removeQueries({ queryKey: listUserOrgsQueryKey(ORGS_OF_THE_USER) });
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

/**
 * Ends the session, then opens the sign-in page. A full load, and not a
 * navigation of the router, clears the data of the old session from the
 * cache of the queries. Whether the server ended it: without that, the page
 * stays and says so.
 */
export async function signOutAndLeave(): Promise<boolean> {
  const hasSignedOut = await signOut();
  if (hasSignedOut) {
    window.location.assign("/sign-in");
  }
  return hasSignedOut;
}
