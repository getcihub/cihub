import { useEffect } from "react";

import { type Me, membershipOf } from "@/domain/session/session";
import { keepLastOrg } from "@/lib/preferences";

/** Keeps `org` as the organization that `/` opens next time, once the person has a role in it. */
export function useRememberOrg(me: Me, org: string): void {
  const membership = membershipOf(me, org);

  useEffect(() => {
    if (membership) {
      keepLastOrg(membership.slug);
    }
  }, [membership]);
}
