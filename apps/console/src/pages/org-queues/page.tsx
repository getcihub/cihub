import { useMutation, useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { getRouteApi } from "@tanstack/react-router";
import { useDeferredValue, useEffect, useRef, useState } from "react";

import { getUserOptions, syncUserMutation } from "@/client/@tanstack/react-query.gen";
import type { Repo, RepoList as RepoListOfOrg, User } from "@/client/types.gen";
import {
  Button,
  buttonVariants,
  Card,
  EmptyState,
  Input,
  Page,
  QueueIllustration,
  SearchIcon,
  SectionCount,
  SectionHead,
  SectionTitle,
  Spinner,
} from "@cihub/ui";
import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { Failure } from "@/components/session/parts";
import { needsAttention } from "@/domain/queue/attention";
import { isListWaiting, readReposAgain, reposOptions } from "@/domain/setup/queries";
import { useMe } from "@/hooks/use-me";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useReadAgain } from "@/hooks/use-read-again";
import { QueueTable } from "./components/queue-table";
import { FirstQueue } from "./components/first-queue";
import { StartList, startRowId } from "./components/start-list";
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
  const me = useMe();
  const queues = useOrgQueues(org);
  const mine = useMinePrs(org);
  const now = useNow();
  // A new search keeps the page, and its field, while its list loads.
  const search = useDeferredValue(q);
  const isConnecting = connecting !== undefined;
  const queryClient = useQueryClient();
  const firstRead = useReadAgain((data: User | undefined) => !data?.synced_at);
  const { data: user } = useSuspenseQuery({
    ...getUserOptions(),
    refetchInterval: firstRead.refetchInterval,
  });
  const isFirstRead = !user.synced_at;
  const [hasAskedAgain, setHasAskedAgain] = useState(false);
  const wait = useReadAgain((data: RepoListOfOrg | undefined) =>
    isListWaiting(data, isConnecting || isFirstRead || hasAskedAgain),
  );
  const { data: all } = useSuspenseQuery(reposOptions(org, undefined));
  const { data: list } = useSuspenseQuery({
    ...reposOptions(org, search),
    refetchInterval: wait.refetchInterval,
  });
  const isWaiting = isListWaiting(list, isConnecting || isFirstRead || (hasAskedAgain && !wait.isOver));
  const hasGivenUp = isWaiting && wait.isOver;
  const isReading = !isConnecting && isWaiting && !hasGivenUp && list.repos.length === 0;
  const hasNoRepository = list.repos.length === 0 && !isReading && !search;
  const sync = useMutation({
    ...syncUserMutation(),
    onSettled: () => readReposAgain(queryClient, org),
    onSuccess: () => {
      setHasAskedAgain(true);
      wait.restart();
    },
  });
  const wasFirstRead = useRef(isFirstRead);
  const hasQueue = (repo: Repo) => queues.some((queue) => queue.repo === repo.name);
  const needsSomeone = (repo: Repo) =>
    queues.some((queue) => queue.repo === repo.name && needsAttention(queue));
  const withQueue = list.repos
    .filter(hasQueue)
    .sort((a, b) => Number(needsSomeone(b)) - Number(needsSomeone(a)));
  const withoutQueue = list.repos.filter((repo) => !hasQueue(repo));
  const isFirstQueue = queues.length === 0 && all.count > 0;
  const verdictOf = useSetupVerdicts(all.repos);

  useEffect(() => {
    if (isConnecting && !isWaiting) {
      void navigate({ search: (kept) => ({ ...kept, connecting: undefined }), replace: true });
    }
  }, [isConnecting, isWaiting, navigate]);

  // The list read last may predate the end of the first read.
  useEffect(() => {
    if (wasFirstRead.current && !isFirstRead) {
      void readReposAgain(queryClient, org);
    }
    wasFirstRead.current = isFirstRead;
  }, [isFirstRead, queryClient, org]);

  return (
    <Page>
      <OverviewHead where={org} title="Repositories">
        {(all.count > SEARCHED_FROM || q) && (
          <span className="relative flex items-center">
            <SearchIcon className="pointer-events-none absolute left-2.25 size-3.5 text-muted-foreground" />
            <Input
              className="h-7 w-65 pr-2.5 pl-7 text-base"
              type="search"
              aria-label="Search the repositories"
              placeholder={`Search ${String(all.count)} repositories`}
              defaultValue={q ?? ""}
              onChange={(event) => {
                const value = event.target.value.trim();
                void navigate({ search: (kept) => ({ ...kept, q: value || undefined }), replace: true });
              }}
            />
          </span>
        )}
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
          onSync={wait.restart}
        />
      )}
      {list.repos.length === 0 && !isReading && search && (
        <Card>
          <p className="px-4 py-6 text-md text-muted-foreground">No repository matches “{search}”.</p>
        </Card>
      )}
      {hasNoRepository && (
        <Card>
          <EmptyState
            illustration={<QueueIllustration />}
            title="No repository yet"
            actions={
              <>
                {me.app_install_url && (
                  <a className={buttonVariants({ variant: "primary" })} href={me.app_install_url}>
                    Add repositories
                  </a>
                )}
                <Button
                  isPending={sync.isPending}
                  pendingText="Checking"
                  onClick={() => {
                    sync.mutate({});
                  }}
                >
                  Check again
                </Button>
              </>
            }
          >
            The App reaches no repository of {org} yet: it reaches only the repositories that an owner chose
            for it.
          </EmptyState>
        </Card>
      )}
      {sync.isError && <Failure className="mb-0">The check failed. Try again in a moment.</Failure>}
      {!hasNoRepository && (
        <p className="text-md text-muted-foreground">
          Missing a repository? The App reaches only the repositories that an owner chose for it.{" "}
          {me.app_install_url && <a href={me.app_install_url}>Add repositories</a>}
        </p>
      )}
    </Page>
  );
}

/** The wait for a read of the repositories that the person reaches: the first one, or one that they asked for. */
function Reading({ org, isFirstRead }: { org: string; isFirstRead: boolean }) {
  return (
    <Card className="flex items-start gap-3 p-4" role="status">
      <Spinner className="mt-1 size-4" />
      <div>
        <h2 className="text-xl font-semibold">Reading your repositories</h2>
        <p className="text-md text-secondary-foreground">
          {isFirstRead
            ? `One moment: CIHub reads on GitHub the repositories of ${org} that your account reaches.`
            : `CIHub reads again on GitHub the repositories of ${org} that your account reaches.`}
        </p>
      </div>
    </Card>
  );
}

/** The wait after an install: the server signed the person in again and connected the organization. */
function Connecting({ org, isUpdate }: { org: string; isUpdate: boolean }) {
  return (
    <Card className="flex items-start gap-3 p-4" role="status">
      <Spinner className="mt-1 size-4" />
      <div>
        <h2 className="text-xl font-semibold">{isUpdate ? `Updating ${org}` : `Connecting ${org}`}</h2>
        <p className="text-md text-secondary-foreground">
          {isUpdate
            ? "The repositories of the App changed on GitHub: CIHub reads them and their setup."
            : "One moment: CIHub reads the repositories of the App and their setup."}
        </p>
        <p className="mt-1 text-md text-muted-foreground">
          The link that brought you here proves nothing by itself: CIHub asked GitHub which installations your
          account can reach.
        </p>
      </div>
    </Card>
  );
}
