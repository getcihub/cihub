import { useSuspenseQueries } from "@tanstack/react-query";

import { type Me, instanceOptions, meOf, userOptions, userOrgsOptions } from "@/domain/session/queries";

/** The person of the session, from the reads of `readMe` in the cache. */
export function useMe(): Me {
  const [user, orgs, instance] = useSuspenseQueries({
    queries: [userOptions(), userOrgsOptions(), instanceOptions()],
  });
  return meOf(user.data, orgs.data, instance.data);
}
