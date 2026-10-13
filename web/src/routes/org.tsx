import type { QueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";

import { minePrsReads } from "@/domain/mine/queries";
import { orgQueuesOptions } from "@/domain/queue/queries";
import { forgetMe, isProblem, readMe } from "@/domain/session/session";
import { startReads } from "@/lib/queryClient";
import { OrgLayout, OrgLayoutError } from "@/pages/OrgLayout";
import { OrgLayoutPending } from "@/pages/skeletons";

type Access = "granted" | "lost" | "signed_out";

/** Reads the queues of `org` into the cache; the API answers 404 where the person has no role. */
async function accessTo(queryClient: QueryClient, org: string): Promise<Access> {
  try {
    await queryClient.query(orgQueuesOptions(org));
    return "granted";
  } catch (error) {
    if (isProblem(error, "org_not_found")) {
      return "lost";
    }
    if (isProblem(error, "unauthorized")) {
      return "signed_out";
    }
    throw error;
  }
}

export const Route = createFileRoute("/orgs/$org")({
  // The two reads leave together; the components read them from the cache.
  loader: async ({ context: { queryClient }, params, location }) => {
    const [me, access] = await Promise.all([readMe(queryClient), accessTo(queryClient, params.org)]);

    if (!me || access === "signed_out") {
      forgetMe(queryClient);
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: location.href } });
    }

    if (access === "granted") {
      // The PRs of the person give the title of the tab on every page of the organization.
      const mine = minePrsReads(params.org);
      await startReads(queryClient.query(mine.live), queryClient.query(mine.merged));
    }

    return { hasAccess: access === "granted" };
  },
  component: OrgLayout,
  pendingComponent: OrgLayoutPending,
  errorComponent: OrgLayoutError,
});
