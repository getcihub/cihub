import type { QueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";

import { minePrsReads } from "@/domain/mine/queries";
import { failureOf, isProblem } from "@/domain/problem";
import { orgQueuesOptions } from "@/domain/queue/queries";
import { forgetMe, readMe } from "@/domain/session/session";
import { OrgLayoutError } from "./error";
import { OrgLayout } from "./layout";
import { OrgLayoutLoading } from "./loading";

type Access = "granted" | "lost" | "signed_out";

/** Reads the queues of `org` into the cache; the API answers 404 where the person has no role. */
async function accessTo(queryClient: QueryClient, org: string): Promise<Access> {
  try {
    await queryClient.query(orgQueuesOptions(org));
    return "granted";
  } catch (thrown) {
    const failure = failureOf(thrown);
    if (isProblem(failure, "org_not_found")) {
      return "lost";
    }
    if (isProblem(failure, "unauthorized")) {
      return "signed_out";
    }
    throw thrown;
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
      const mine = minePrsReads(params.org, me.login);
      await Promise.allSettled([queryClient.query(mine.live), queryClient.query(mine.merged)]);
    }

    return { hasAccess: access === "granted" };
  },
  component: OrgLayout,
  pendingComponent: OrgLayoutLoading,
  errorComponent: OrgLayoutError,
});
