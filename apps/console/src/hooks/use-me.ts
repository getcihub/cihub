import { useSuspenseQueries } from "@tanstack/react-query";

import { type Me, meOf, readsOfMe } from "@/domain/session/session";

/** The person of the session, from the reads of `readMe` in the cache. */
export function useMe(): Me {
  const [user, orgs, instance] = useSuspenseQueries({ queries: readsOfMe() });
  return meOf(user.data, orgs.data, instance.data);
}
