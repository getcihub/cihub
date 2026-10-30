import type { QueryClient } from "@tanstack/react-query";

import { getInstanceOptions, getUserOptions, listUserOrgsOptions } from "@/client/@tanstack/react-query.gen";
import type { Instance, Org, OrgList, User } from "@/client/types.gen";
import { failureOf, isProblem } from "@/domain/problem";

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
}

/** The membership of the person in `org`, whatever the case of its slug; `undefined` without a role in it. */
export function membershipOf(me: Me, org: string): Org | undefined {
  return me.orgs.find((membership) => membership.slug.toLowerCase() === org.toLowerCase());
}

/** The read of the person of the session. */
export function userOptions(): ReturnType<typeof getUserOptions> {
  return getUserOptions();
}

/** The read of the organizations of the person: a person has a few, one page holds them all. */
export function userOrgsOptions(): ReturnType<typeof listUserOrgsOptions> {
  return listUserOrgsOptions({ query: { limit: 100 } });
}

/** The read of the instance: its version, and the address that installs its App. */
export function instanceOptions(): ReturnType<typeof getInstanceOptions> {
  return getInstanceOptions();
}

/** The person of the session, or `null` without a session; another failure of a read throws. */
export async function readMe(queryClient: QueryClient): Promise<Me | null> {
  const reads = [
    queryClient.query(userOptions()),
    queryClient.query(userOrgsOptions()),
    queryClient.query(instanceOptions()),
  ] as const;
  const settled = await Promise.allSettled(reads);

  const isSignedOut = settled.some(
    (read) => read.status === "rejected" && isProblem(failureOf(read.reason), "unauthorized"),
  );
  if (isSignedOut) {
    return null;
  }

  return meOf(...(await Promise.all(reads)));
}

/** The person of the session, from the answers of its three reads. */
export function meOf(user: User, orgs: OrgList, instance: Instance): Me {
  return {
    login: user.login,
    name: user.name,
    avatar_url: user.avatar_url,
    synced_at: user.synced_at,
    orgs: orgs.orgs,
    version: instance.version,
    app_install_url: instance.app_install_url,
  };
}
