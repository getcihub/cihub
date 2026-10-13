import { type ErrorComponentProps, getRouteApi } from "@tanstack/react-router";

import { ServerError } from "@/components/errors/ServerError";
import { MinePrs } from "@/components/mine/Mine";
import { NoQueueYet } from "@/components/queue/NoQueueYet";
import { OverviewHead } from "@/components/queue/OverviewHead";
import { QueueCards } from "@/components/queue/QueueCards";
import { useMinePrs } from "@/domain/mine/queries";
import { useOrgQueues } from "@/domain/queue/queries";
import { useMe } from "@/domain/session/session";
import { Card } from "@/ui/card";
import { Page } from "@/ui/page";

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

/** The queues could not be read: the head of the page stays, with what the server answered. */
export function OrgQueuesPageError({ error, reset }: ErrorComponentProps) {
  const { org } = route.useParams();

  return (
    <Page>
      <OverviewHead where={org} />
      <Card>
        <ServerError error={error} reset={reset} />
      </Card>
    </Page>
  );
}
