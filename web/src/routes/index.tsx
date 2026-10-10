import { createFileRoute, redirect } from "@tanstack/react-router";

import { lastOrgSlug } from "../preferences";
import { readMe } from "../session";

/** The queues of the organization that the person opened last, else of their first one. */
export const Route = createFileRoute("/")({
  beforeLoad: async ({ context }) => {
    const me = await readMe(context.queryClient);
    const last = lastOrgSlug()?.toLowerCase();
    const org = me?.orgs.find((membership) => membership.slug.toLowerCase() === last) ?? me?.orgs[0];

    if (!org) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: "/" } });
    }

    // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
    throw redirect({ to: "/orgs/$org", params: { org: org.slug } });
  },
});
