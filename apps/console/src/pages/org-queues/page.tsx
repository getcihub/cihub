import { getRouteApi } from "@tanstack/react-router";
import { useDeferredValue, useEffect } from "react";

import type { Repo } from "@/client/types.gen";
import { ErrorAlert } from "@/components/errors/error-alert";
import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { Card } from "@cihub/ui/components/card";
import { Page } from "@cihub/ui/components/page";
import { SectionCount, SectionHead, SectionTitle } from "@cihub/ui/components/section";
import { needsAttention } from "@/domain/queue/attention";
import { useMe } from "@/hooks/use-me";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { FirstQueue } from "./components/first-queue";
import { NoRepository } from "./components/no-repository";
import { QueueTable } from "./components/queue-table";
import { RepoSearch } from "./components/repo-search";
import { StartList, startRowId } from "./components/start-list";
import { Connecting, Reading } from "./components/wait-cards";
import { useRepoList } from "./use-repo-list";
import { isBlockedVerdict, useSetupVerdicts } from "./use-setup-verdicts";

const route = getRouteApi("/orgs/$org/");

/** A list longer than this gets a search. */
const SEARCHED_FROM = 5;

/**
 * The repositories of an organization, after the PRs of the person: those with a queue first, one row per queue,
 * the ones whose queues need someone at the top; then the others, each one opening in place on what it needs to
 * start. After an install, the Setup URL opens it with `?connecting=`: the page says that CIHub connects the
 * organization while the first reads wait, then drops the parameter. Before the first read of the repositories
 * of the person ends, an empty list waits for it too; after it, "Check again" asks for another one.
 */
export function OrgQueuesPage() {
  const { org } = route.useParams();
  const { q, connecting, open } = route.useSearch();
  const navigate = route.useNavigate();
  // A new search keeps the page, and its field, while its list loads.
  const search = useDeferredValue(q);
  const isConnecting = connecting !== undefined;
  const now = useNow();

  const me = useMe();
  const queues = useOrgQueues(org);
  const mine = useMinePrs(org);
  const { all, list, isFirstRead, isWaiting, hasGivenUp, isReading, checkAgain, waitAgain } = useRepoList(
    org,
    search,
    isConnecting,
  );
  const verdictOf = useSetupVerdicts(all.repos);

  const hasQueue = (repo: Repo) => queues.some((queue) => queue.repo === repo.name);
  const needsSomeone = (repo: Repo) =>
    queues.some((queue) => queue.repo === repo.name && needsAttention(queue));
  const withQueue = list.repos
    .filter(hasQueue)
    .sort((a, b) => Number(needsSomeone(b)) - Number(needsSomeone(a)));
  const withoutQueue = list.repos.filter((repo) => !hasQueue(repo));
  const isFirstQueue = queues.length === 0 && all.count > 0;
  const hasNoRepository = list.repos.length === 0 && !isReading && !search;

  useEffect(() => {
    if (isConnecting && !isWaiting) {
      void navigate({ search: (kept) => ({ ...kept, connecting: undefined }), replace: true });
    }
  }, [isConnecting, isWaiting, navigate]);

  return (
    <Page>
      <OverviewHead where={org} title="Repositories">
        {(all.count > SEARCHED_FROM || q) && <RepoSearch count={all.count} />}
      </OverviewHead>
      {isConnecting && isWaiting && !hasGivenUp && (
        <Connecting org={org} isUpdate={connecting === "update"} />
      )}
      {isReading && <Reading org={org} isFirstRead={isFirstRead} />}
      {hasGivenUp && (
        <p className="text-md text-muted-foreground" role="status">
          The reads take longer than a minute: the page stopped waiting. Reload it in a moment.
        </p>
      )}
      {isFirstQueue && (
        <FirstQueue
          org={org}
          count={all.count}
          repos={all.repos}
          verdictOf={verdictOf}
          open={open}
          installUrl={me.app_install_url}
          onStart={(repo) => {
            void navigate({
              search: (kept) => ({ ...kept, open: repo.name }),
              replace: true,
              resetScroll: false,
            }).then(() => {
              document.getElementById(startRowId(repo))?.scrollIntoView({ block: "nearest" });
            });
          }}
        />
      )}
      <MinePrs org={org} prs={mine} queues={queues} />
      {withQueue.length > 0 && (
        <>
          <SectionHead>
            <SectionTitle>
              With a queue<SectionCount>{withQueue.length}</SectionCount>
            </SectionTitle>
          </SectionHead>
          <QueueTable org={org} repos={withQueue} queues={queues} now={now} />
        </>
      )}
      {withoutQueue.length > 0 && (
        <StartList
          org={org}
          title={withQueue.length > 0 || search ? "Not started" : "Repositories"}
          repos={withoutQueue}
          open={open}
          isSearch={Boolean(search)}
          isBlocked={isFirstQueue && !search ? (repo) => isBlockedVerdict(verdictOf(repo)) : null}
          onSync={waitAgain}
        />
      )}
      {list.repos.length === 0 && !isReading && search && (
        <Card>
          <p className="px-4 py-6 text-md text-muted-foreground">No repository matches “{search}”.</p>
        </Card>
      )}
      {hasNoRepository && <NoRepository org={org} installUrl={me.app_install_url} checkAgain={checkAgain} />}
      {checkAgain.hasFailed && (
        <ErrorAlert className="mb-0">The check failed. Try again in a moment.</ErrorAlert>
      )}
      {!hasNoRepository && (
        <p className="text-md text-muted-foreground">
          Missing a repository? The App reaches only the repositories that an owner chose for it.{" "}
          {me.app_install_url && <a href={me.app_install_url}>Add repositories</a>}
        </p>
      )}
    </Page>
  );
}
