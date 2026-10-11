import { useSuspenseQuery, type QueryClient } from "@tanstack/react-query";
import { Outlet, createFileRoute, redirect, useParams } from "@tanstack/react-router";
import { useEffect } from "react";

import { listOrgQueuesOptions, meOptions, meQueryKey } from "../client/@tanstack/react-query.gen";
import type { Me } from "../client/types.gen";
import { Frame } from "../frame/Frame";
import { minePrsOptions, whereOf } from "../mine/reads";
import { OrgLostPage } from "../pages/OrgLostPage";
import { keepLastOrg } from "../preferences";
import { isProblem, readMe } from "../session";

type Access = "granted" | "lost" | "signed_out";

/** Reads the queues of `org` into the cache; the API answers 404 where the person has no role. */
async function accessTo(queryClient: QueryClient, org: string): Promise<Access> {
  try {
    await queryClient.query(listOrgQueuesOptions({ path: { org } }));
    return "granted";
  } catch (error) {
    if (isProblem(error, "not_found")) {
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
      // A `/me` still fresh in the cache would send the person back here from the sign-in.
      queryClient.removeQueries({ queryKey: meQueryKey() });
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- the router stops on a thrown redirect
      throw redirect({ to: "/sign-in", search: { next: location.href } });
    }

    if (access === "granted") {
      // The PRs of the person give the title of the tab on every page of the organization.
      await queryClient.query(minePrsOptions(params.org, me.login)).catch(() => undefined);
    }

    return { hasAccess: access === "granted" };
  },
  component: OrgLayout,
});

function OrgLayout() {
  const { hasAccess } = Route.useLoaderData();
  const { org } = Route.useParams();
  const { data: me } = useSuspenseQuery(meOptions());

  if (!hasAccess) {
    return (
      <Frame me={me} org={org} queues={[]}>
        <OrgLostPage me={me} org={org} />
      </Frame>
    );
  }

  return <OrgFrame me={me} org={org} />;
}

function OrgFrame({ me, org }: { me: Me; org: string }) {
  const { data } = useSuspenseQuery(listOrgQueuesOptions({ path: { org } }));
  const { data: mine } = useSuspenseQuery(minePrsOptions(org, me.login));
  const first = mine.prs[0];
  const firstQueue = data.queues.find(
    (candidate) => candidate.repo === first?.repo && candidate.queue.name === first.queue,
  )?.queue;
  const title = first
    ? `#${String(first.item.pr_number)} ${whereOf(first, firstQueue?.state === "paused", firstQueue?.target_branch ?? first.queue)} · CIHub`
    : "CIHub";
  const { repo, queue } = useParams({ strict: false });
  const membership = me.orgs.find((candidate) => candidate.slug.toLowerCase() === org.toLowerCase());
  // The frame names only a repository and a queue that exist: a wrong address shows "not found" under it.
  const knownRepo = data.queues.some((candidate) => candidate.repo === repo) ? repo : undefined;
  const knownQueue = data.queues.some(
    (candidate) => candidate.repo === repo && candidate.queue.name === queue,
  )
    ? queue
    : undefined;

  useEffect(() => {
    if (membership) {
      keepLastOrg(membership.slug);
    }
  }, [membership]);

  // The tab says where the first PR of the person is, from any page.
  useEffect(() => {
    document.title = title;
  }, [title]);

  return (
    <Frame me={me} org={org} queues={data.queues} repo={knownRepo} queue={knownQueue}>
      <Outlet />
    </Frame>
  );
}
