import { useSuspenseQuery } from "@tanstack/react-query";
import { getRouteApi } from "@tanstack/react-router";
import { useDeferredValue, useEffect } from "react";

import type { RepoList as RepoListOfOrg } from "@/client/types.gen";
import { MinePrs } from "@/components/mine/mine";
import { OverviewHead } from "@/components/queue/overview-head";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Page } from "@/components/ui/page";
import { Spinner } from "@/components/ui/spinner";
import { isListWaiting, reposOptions } from "@/domain/setup/queries";
import { useMe } from "@/hooks/use-me";
import { useMinePrs } from "@/hooks/use-mine-prs";
import { useNow } from "@/hooks/use-now";
import { useOrgQueues } from "@/hooks/use-org-queues";
import { useReadAgain } from "@/hooks/use-read-again";
import { RepoList } from "./components/repo-list";

const route = getRouteApi("/orgs/$org/");

/** A list longer than this gets a search. */
const SEARCHED_FROM = 5;

/**
 * The repositories of an organization, each with its setup and its queues, after the PRs of the person. After an
 * install, the Setup URL opens it with `?connecting=`: the page says that CIHub connects the organization while
 * the first reads wait, then drops the parameter.
 */
export function OrgQueuesPage() {
  const { org } = route.useParams();
  const { q, connecting } = route.useSearch();
  const navigate = route.useNavigate();
  const me = useMe();
  const queues = useOrgQueues(org);
  const mine = useMinePrs(org);
  const now = useNow();
  // A new search keeps the page, and its field, while its list loads.
  const search = useDeferredValue(q);
  const isConnecting = connecting !== undefined;
  const wait = useReadAgain((data: RepoListOfOrg | undefined) => isListWaiting(data, isConnecting));
  const { data: list } = useSuspenseQuery({
    ...reposOptions(org, search),
    refetchInterval: wait.refetchInterval,
  });
  const isWaiting = isListWaiting(list, isConnecting);
  const hasGivenUp = isWaiting && wait.isOver;

  useEffect(() => {
    if (isConnecting && !isWaiting) {
      void navigate({ search: (kept) => ({ ...kept, connecting: undefined }), replace: true });
    }
  }, [isConnecting, isWaiting, navigate]);

  return (
    <Page>
      <OverviewHead where={org} title="Repositories" />
      {isConnecting && isWaiting && !hasGivenUp && (
        <Connecting org={org} isUpdate={connecting === "update"} />
      )}
      {hasGivenUp && (
        <p className="mb-4 text-md text-muted-foreground" role="status">
          The reads take longer than a minute: the page stopped waiting. Reload it in a moment.
        </p>
      )}
      <MinePrs org={org} prs={mine} queues={queues} />
      {(list.count > SEARCHED_FROM || q) && (
        <Input
          className="mb-3 h-7 w-65 px-2.5 text-base"
          type="search"
          aria-label="Search the repositories"
          placeholder={`Search ${String(list.count)} repositories`}
          defaultValue={q ?? ""}
          onChange={(event) => {
            const value = event.target.value.trim();
            void navigate({ search: (kept) => ({ ...kept, q: value || undefined }), replace: true });
          }}
        />
      )}
      <RepoList
        org={org}
        repos={list.repos}
        queues={queues}
        now={now}
        empty={
          search
            ? `No repository matches “${search}”.`
            : "The App reaches no repository of this organization yet."
        }
      />
      <p className="mt-4 text-md text-muted-foreground">
        Missing a repository? The App reaches only the repositories that an owner chose for it.{" "}
        {me.app_install_url && <a href={me.app_install_url}>Add repositories</a>}
      </p>
    </Page>
  );
}

/** The wait after an install: the server signed the person in again and connected the organization. */
function Connecting({ org, isUpdate }: { org: string; isUpdate: boolean }) {
  return (
    <Card className="mb-4 flex items-start gap-3 p-4" role="status">
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
