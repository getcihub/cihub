import { type ErrorComponentProps, Outlet, getRouteApi, useParams } from "@tanstack/react-router";

import { OrgLostPage } from "@/components/errors/OrgLostPage";
import { ServerErrorPage } from "@/components/errors/ServerErrorPage";
import { Frame, TopBar, TopBarLogo } from "@/components/frame/Frame";
import { MoveNotice } from "@/components/mine/MoveNotice";
import { useMinePrs } from "@/domain/mine/queries";
import { tabTitleOf } from "@/domain/pr/words";
import { useQueueNews } from "@/domain/queue/live";
import { useOrgQueues } from "@/domain/queue/queries";
import { type Me, useMe, useRememberOrg } from "@/domain/session/session";
import { useDocumentTitle } from "@/lib/document";

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

/** The person or the queues of the organization could not be read: no frame to show, only what the server answered. */
export function OrgLayoutError(props: ErrorComponentProps) {
  return (
    <>
      <TopBar>
        <TopBarLogo />
      </TopBar>
      <ServerErrorPage {...props} />
    </>
  );
}
