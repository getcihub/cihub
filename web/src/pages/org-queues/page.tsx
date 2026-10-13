import { getRouteApi } from "@tanstack/react-router";

import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { QueueCards } from "@/components/queue/queue-cards";
import { Page } from "@/components/ui/page";
import { useMe } from "@/hooks/use-me";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { NoQueueYet } from "@/pages/org-queues/components/no-queue-yet";

const route = getRouteApi("/orgs/$org/");

/** The queues of an organization, after the PRs of the person; the way to a first queue when it has none. */
export function OrgQueuesPage() {
  const { org } = route.useParams();
  const me = useMe();
  const queues = useOrgQueues(org);
  const mine = useMinePrs(org);

  return (
    <Page>
      <OverviewHead where={org} />
      {queues.length > 0 ? (
        <>
          <MinePrs org={org} prs={mine} queues={queues} />
          <QueueCards org={org} queues={queues} />
        </>
      ) : (
        <NoQueueYet org={org} installUrl={me.app_install_url} />
      )}
    </Page>
  );
}
