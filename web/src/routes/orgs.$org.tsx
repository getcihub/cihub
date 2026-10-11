import { useSuspenseQuery, type QueryClient } from "@tanstack/react-query";
import { Outlet, createFileRoute, redirect, useParams } from "@tanstack/react-router";
import { useEffect } from "react";

import { Frame } from "../frame/Frame";
import { useQueueNews } from "../live";
import { MoveNotice } from "../mine/MoveNotice";
import { minePrsReads, useMinePrs, whereOf } from "../mine/reads";
import { OrgLostPage } from "../pages/OrgLostPage";
import { keepLastOrg } from "../preferences";
import { type Me, forgetMe, isProblem, readMe, useMe } from "../session";
import { orgQueuesOptions } from "../queue/reads";

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
      const ignore = () => undefined;
      await Promise.all([
        queryClient.query(mine.live).catch(ignore),
        queryClient.query(mine.merged).catch(ignore),
      ]);
    }

    return { hasAccess: access === "granted" };
  },
  component: OrgLayout,
});

function OrgLayout() {
  const { hasAccess } = Route.useLoaderData();
  const { org } = Route.useParams();
  const me = useMe();

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
  const { data } = useSuspenseQuery(orgQueuesOptions(org));
  const mine = useMinePrs(org);
  const [first] = mine;
  const firstQueue = data.queues.find(
    (candidate) => candidate.repo === first?.repo && candidate.name === first.queue,
  );
  const title = first
    ? `#${String(first.number)} ${whereOf(first, firstQueue?.state === "paused", firstQueue?.target_branch ?? first.queue)} · CIHub`
    : "CIHub";
  const { repo, queue } = useParams({ strict: false });
  const membership = me.orgs.find((candidate) => candidate.slug.toLowerCase() === org.toLowerCase());
  // The frame names only a repository and a queue that exist: a wrong address shows "not found" under it.
  const knownRepo = data.queues.some((candidate) => candidate.repo === repo) ? repo : undefined;
  const knownQueue = data.queues.some((candidate) => candidate.repo === repo && candidate.name === queue)
    ? queue
    : undefined;

  useEffect(() => {
    if (membership) {
      keepLastOrg(membership.slug);
    }
  }, [membership]);

  useQueueNews(org);

  // The tab says where the first PR of the person is, from any page.
  useEffect(() => {
    document.title = title;
  }, [title]);

  return (
    <Frame me={me} org={org} queues={data.queues} repo={knownRepo} queue={knownQueue}>
      <Outlet />
      <MoveNotice org={org} queues={data.queues} />
    </Frame>
  );
}
