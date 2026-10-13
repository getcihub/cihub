import { createFileRoute, redirect } from "@tanstack/react-router";

import { ServerErrorPage } from "@/components/errors/server-error-page";
import { PendingPage } from "@/components/frame/pending-page";
import { membershipOf, readMe } from "@/domain/session/session";
import { lastOrgSlug } from "@/lib/preferences";

/** The queues of the organization that the person opened last, else of their first one. */
export const Route = createFileRoute("/")({
  beforeLoad: async ({ context }) => {
    const me = await readMe(context.queryClient);
    const last = lastOrgSlug();
    const lastOrg = me && last ? membershipOf(me, last) : undefined;
    const org = lastOrg ?? me?.orgs[0];

    if (!org) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: "/" } });
    }

    // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
    throw redirect({ to: "/orgs/$org", params: { org: org.slug } });
  },
  pendingComponent: PendingPage,
  errorComponent: ServerErrorPage,
});
