import { useSuspenseQuery } from "@tanstack/react-query";
import { getRouteApi } from "@tanstack/react-router";
import { useDeferredValue, useEffect } from "react";

import type { Repo, RepoList as RepoListOfOrg } from "@/client/types.gen";
import { QueuesIcon } from "@/components/icons/queues-icon";
import { SearchIcon } from "@/components/icons/search-icon";
import { MinePrs } from "@/components/mine/mine";
import { COMMAND } from "@/components/queue/copy-command";
import { OverviewHead } from "@/components/queue/overview-head";
import { Alert, AlertContent, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Page } from "@/components/ui/page";
import { SectionCount, SectionHead, SectionTitle } from "@/components/ui/section";
import { Spinner } from "@/components/ui/spinner";
import { needsAttention } from "@/domain/queue/attention";
import { isListWaiting, reposOptions } from "@/domain/setup/queries";
import { useMe } from "@/hooks/use-me";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useReadAgain } from "@/hooks/use-read-again";
import { plural } from "@/lib/text";
import { QueueTable } from "./components/queue-table";
import { StartList } from "./components/start-list";

const route = getRouteApi("/orgs/$org/");

/** A list longer than this gets a search. */
const SEARCHED_FROM = 5;

/**
 * The repositories of an organization, after the PRs of the person: those with a queue first, one row per queue,
 * the ones whose queues need someone at the top; then the others, each one opening in place on what it needs to
 * start. After an install, the Setup URL opens it with `?connecting=`: the page says that CIHub connects the
 * organization while the first reads wait, then drops the parameter.
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
  const wait = useReadAgain((data: RepoListOfOrg | undefined) => isListWaiting(data, isConnecting));
  const { data: all } = useSuspenseQuery(reposOptions(org, undefined));
  const { data: list } = useSuspenseQuery({
    ...reposOptions(org, search),
    refetchInterval: wait.refetchInterval,
  });
  const isWaiting = isListWaiting(list, isConnecting);
  const hasGivenUp = isWaiting && wait.isOver;
  const hasQueue = (repo: Repo) => queues.some((queue) => queue.repo === repo.name);
  const needsSomeone = (repo: Repo) =>
    queues.some((queue) => queue.repo === repo.name && needsAttention(queue));
  const withQueue = list.repos
    .filter(hasQueue)
    .sort((a, b) => Number(needsSomeone(b)) - Number(needsSomeone(a)));
  const withoutQueue = list.repos.filter((repo) => !hasQueue(repo));

  useEffect(() => {
    if (isConnecting && !isWaiting) {
      void navigate({ search: (kept) => ({ ...kept, connecting: undefined }), replace: true });
    }
  }, [isConnecting, isWaiting, navigate]);

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
      {hasGivenUp && (
        <p className="text-md text-muted-foreground" role="status">
          The reads take longer than a minute: the page stopped waiting. Reload it in a moment.
        </p>
      )}
      {queues.length === 0 && all.count > 0 && <NoQueueYet org={org} count={all.count} />}
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
        <>
          <SectionHead>
            <SectionTitle>
              {withQueue.length > 0 || search ? "Not started" : "Repositories"}
              <SectionCount>{withoutQueue.length}</SectionCount>
            </SectionTitle>
          </SectionHead>
          <StartList
            org={org}
            repos={withoutQueue}
            open={open}
            isSearch={Boolean(search)}
            onSync={wait.restart}
          />
        </>
      )}
      {list.repos.length === 0 && (
        <Card>
          <p className="px-4 py-6 text-md text-muted-foreground">
            {search
              ? `No repository matches “${search}”.`
              : "The App reaches no repository of this organization yet."}
          </p>
        </Card>
      )}
      <p className="text-md text-muted-foreground">
        Missing a repository? The App reaches only the repositories that an owner chose for it.{" "}
        {me.app_install_url && <a href={me.app_install_url}>Add repositories</a>}
      </p>
    </Page>
  );
}

/** The organization has no queue yet: where to start. */
function NoQueueYet({ org, count }: { org: string; count: number }) {
  return (
    <Alert isBanner variant="info" role="status">
      <QueuesIcon />
      <AlertContent>
        <AlertTitle>No queue yet in {org}.</AlertTitle>
        <AlertDescription>
          The App reaches {plural(count, "repository", "repositories")}. Start with one where pull requests
          merge often: open it below, then comment <code>{COMMAND}</code> on a pull request.
        </AlertDescription>
      </AlertContent>
    </Alert>
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
