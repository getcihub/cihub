// The session of the dashboard: who signs in, and the routes of the server
// outside the API (`specs/API.md`, `specs/SECURITY.md`).
import type { QueryClient } from "@tanstack/react-query";
import { z } from "zod";

import {
  getInstanceOptions,
  getUserOptions,
  getUserQueryKey,
  listUserOrgsOptions,
  listUserOrgsQueryKey,
} from "@/client/@tanstack/react-query.gen";
import { client } from "@/client/client.gen";
import type { Org, User } from "@/client/types.gen";
import { failureOf, isProblem } from "@/domain/problem";

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

/** The person of the session, their organizations, and the instance: `/user`, `/user/orgs` and `/instance`. */
export interface Me {
  login: string;
  name?: string | null;
  avatar_url?: string | null;
  /** When CIHub last read the repositories that the person reaches; `null` until the first read ends. */
  synced_at?: string | null;
  orgs: Org[];
  version: string;
  app_install_url?: string | null;
  app_mention?: string | null;
}

/** The membership of the person in `org`, whatever the case of its slug; `undefined` without a role in it. */
export function membershipOf(me: Me, org: string): Org | undefined {
  return me.orgs.find((membership) => membership.slug.toLowerCase() === org.toLowerCase());
}

/** A person has a few organizations: one page holds them all. */
const ORGS_OF_THE_USER = { query: { limit: 100 } };

/** The reads of the person of the session, which `readMe` starts and `useMe` reads. */
export const readsOfMe = () =>
  [getUserOptions(), listUserOrgsOptions(ORGS_OF_THE_USER), getInstanceOptions()] as const;

/** The person of the session, or `null` without a session; another failure of a read throws. */
export async function readMe(queryClient: QueryClient): Promise<Me | null> {
  const [user, orgs, instance] = readsOfMe();
  const reads = [queryClient.query(user), queryClient.query(orgs), queryClient.query(instance)] as const;
  const settled = await Promise.allSettled(reads);

  if (
    settled.some((read) => read.status === "rejected" && isProblem(failureOf(read.reason), "unauthorized"))
  ) {
    return null;
  }
  return meOf(...(await Promise.all(reads)));
}

export function meOf(
  user: User,
  orgs: { orgs: Org[] },
  instance: { version: string; app_install_url?: string | null; app_mention?: string | null },
): Me {
  return {
    login: user.login,
    name: user.name,
    avatar_url: user.avatar_url,
    synced_at: user.synced_at,
    orgs: orgs.orgs,
    version: instance.version,
    app_install_url: instance.app_install_url,
    app_mention: instance.app_mention,
  };
}

/** The command that submits a PR in a comment: the mention of the App, `@<app>` when the SCM did not give it. */
export function mergeCommandOf(me: Me): string {
  return `${me.app_mention ?? "@<app>"} merge`;
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
