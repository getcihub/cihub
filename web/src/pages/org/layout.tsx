import { Outlet, getRouteApi, useParams } from "@tanstack/react-router";

import { Frame } from "@/components/frame/frame";
import { tabTitleOf } from "@/domain/pr/words";
import { type Me } from "@/domain/session/session";
import { useMe } from "@/hooks/use-me";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { MoveNotice } from "@/pages/org/components/move-notice";
import { OrgLostPage } from "@/pages/org/components/org-lost-page";
import { useDocumentTitle } from "./use-document-title";
import { useQueueNews } from "./use-queue-news";
import { useRememberOrg } from "./use-remember-org";

const route = getRouteApi("/orgs/$org");

/** The pages of an organization in its frame, or the page that says that the person has no access to it. */
export function OrgLayout() {
  const { hasAccess } = route.useLoaderData();
  const { org } = route.useParams();
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
  const queues = useOrgQueues(org);
  const [firstPr] = useMinePrs(org);
  const { repo, queue } = useParams({ strict: false });

  useRememberOrg(me, org);
  useQueueNews(org);
  useDocumentTitle(tabTitleOf(firstPr, queues));

  return (
    <Frame me={me} org={org} queues={queues} repo={repo} queue={queue}>
      <Outlet />
      <MoveNotice org={org} queues={queues} />
    </Frame>
  );
}
