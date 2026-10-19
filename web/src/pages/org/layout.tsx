import { useQuery } from "@tanstack/react-query";
import { Outlet, getRouteApi, useParams } from "@tanstack/react-router";

import { Frame } from "@/components/frame/frame";
import { ErrorIcon } from "@/components/icons/error-icon";
import { Alert, AlertContent, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { tabTitleOf } from "@/domain/pr/words";
import { reposOptions } from "@/domain/setup/queries";
import { type Me, membershipOf } from "@/domain/session/session";
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
  // The picker of the repositories adds those without a queue once they load.
  const repos = useQuery(reposOptions(org, undefined)).data?.repos.map((candidate) => candidate.name);
  const [firstPr] = useMinePrs(org);
  const { repo, queue } = useParams({ strict: false });

  useRememberOrg(me, org);
  useQueueNews(org);
  useDocumentTitle(tabTitleOf(firstPr, queues));

  return (
    <Frame me={me} org={org} queues={queues} repos={repos} repo={repo} queue={queue}>
      {membershipOf(me, org)?.is_suspended && <SuspendedBanner org={org} />}
      <Outlet />
      <MoveNotice org={org} queues={queues} />
    </Frame>
  );
}

/** Every installation of the App on the organization is suspended: the pages still open, and nothing merges. */
function SuspendedBanner({ org }: { org: string }) {
  return (
    <Alert isBanner variant="destructive" role="status" className="mx-6 mt-4">
      <ErrorIcon />
      <AlertContent>
        <AlertTitle>CIHub lost its access to {org}.</AlertTitle>
        <AlertDescription>
          The GitHub App is suspended or removed from this organization. Its queues keep their pull requests
          and their order, and merge nothing until an owner gives the access back on GitHub.
        </AlertDescription>
      </AlertContent>
    </Alert>
  );
}
