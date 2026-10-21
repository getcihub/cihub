import { getRouteApi } from "@tanstack/react-router";

import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { QueueCards } from "@/components/queue/queue-cards";
import { Page } from "@/components/ui/page";
import { SectionCount, SectionHead, SectionTitle } from "@/components/ui/section";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useRepoSetup } from "@/hooks/use-repo-setup";
import { RepoSetup } from "./components/repo-setup";
import { RepoStart } from "./components/repo-start";

const route = getRouteApi("/orgs/$org/repos/$repo/");

/**
 * A repository: the setup of its default branch, which a read follows every 2 seconds while it waits, then its
 * queues, after the PRs of the person in it. Before its first queue, how the queue starts leads the page, and the
 * setup supports it.
 */
export function RepoQueuesPage() {
  const { org, repo: name } = route.useParams();
  const now = useNow();
  const setup = useRepoSetup(org, name);
  const queues = useOrgQueues(org).filter((queue) => queue.repo === name);
  const mine = useMinePrs(org).filter((pr) => pr.repo === name);

  return (
    <Page>
      <OverviewHead where={org} title={name} />
      {queues.length === 0 && <RepoStart {...setup} />}
      <RepoSetup {...setup} now={now} />
      {queues.length > 0 && (
        <>
          <MinePrs org={org} prs={mine} queues={queues} />
          <SectionHead>
            <SectionTitle>
              Queues<SectionCount>{queues.length}</SectionCount>
            </SectionTitle>
          </SectionHead>
          <QueueCards org={org} queues={queues} />
        </>
      )}
    </Page>
  );
}
