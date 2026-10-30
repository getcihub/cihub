import { getRouteApi } from "@tanstack/react-router";
import { Fragment, Suspense, useState } from "react";

import type { Repo, SetupState } from "@/client/types.gen";
import { ArrowIcon } from "@cihub/icons/arrow";
import { buttonVariants } from "@cihub/ui/components/button";
import { Card } from "@cihub/ui/components/card";
import { SectionCount, SectionHead, SectionTitle } from "@cihub/ui/components/section";
import { cn } from "@cihub/ui/lib/cn";
import { SetupStateBadge } from "@/components/setup/setup-state-badge";
import { isSetupBlocked } from "@/domain/setup/queries";
import { plural } from "@/lib/text";
import { StartDetail, StartDetailSkeleton } from "./start-detail";

const route = getRouteApi("/orgs/$org/");

/** Without a search, the list shows this many repositories, and cuts only a longer list than this and two. */
const SHOWN_FIRST = 8;

/** What the button of a closed row says, by the verdict of the setup. */
const ACTIONS: Record<SetupState, string> = {
  checking: "Open",
  started: "Open",
  ready: "Start",
  ready_with_warnings: "Start",
  cannot_merge: "Fix",
  invalid_config: "Fix",
};

/** A part of the list under its own head: the rows of one kind. */
interface Group {
  title: string;
  /** What the head says on its right. */
  hint?: string;
  repos: Repo[];
}

/**
 * The repositories without a queue: each row opens in place on what the repository still needs, in the order of
 * the work. The open row is in the address (`?open=`), so that a link keeps it. Before the first queue of the
 * organization, and without a search, the list splits into the repositories ready to start and the ones that need
 * a fix.
 */
export function StartList({
  org,
  title,
  repos,
  open,
  isSearch,
  isBlocked,
  onSync,
}: {
  org: string;
  /** The head of the list when it does not split. */
  title: string;
  repos: Repo[];
  open: string | undefined;
  /** A search shows every match. */
  isSearch: boolean;
  /** The repository needs a fix before its first queue; `null` keeps the list in one part. */
  isBlocked: ((repo: Repo) => boolean) | null;
  /** A setup is read again: the list waits for it. */
  onSync: () => void;
}) {
  const navigate = route.useNavigate();
  const [isAllShown, setIsAllShown] = useState(false);
  const isCut = !isSearch && !isAllShown && repos.length > SHOWN_FIRST + 2;
  const shown = isCut ? repos.filter((repo, index) => index < SHOWN_FIRST || repo.name === open) : repos;
  const groups: Group[] = isBlocked
    ? [
        { title: "Ready to start", repos: repos.filter((repo) => !isBlocked(repo)) },
        {
          title: "Needs a fix",
          hint: "A point blocks every merge until it is fixed",
          repos: repos.filter(isBlocked),
        },
      ]
    : [{ title, repos }];
  // A part whose rows the cut hides shows once the person asks for all.
  const visible = groups
    .map((group) => ({ ...group, shown: shown.filter((repo) => group.repos.includes(repo)) }))
    .filter((group) => group.shown.length > 0);

  return visible.map((group, index) => (
    <Fragment key={group.title}>
      <SectionHead>
        <SectionTitle>
          {group.title}
          <SectionCount>{group.repos.length}</SectionCount>
        </SectionTitle>
        {group.hint && <span className="ml-auto text-md text-muted-foreground">{group.hint}</span>}
      </SectionHead>
      <Card className="p-0">
        {group.shown.map((repo) => (
          <StartRow
            key={repo.id}
            org={org}
            repo={repo}
            isOpen={repo.name === open}
            onToggle={() => {
              void navigate({
                search: (kept) => ({ ...kept, open: repo.name === open ? undefined : repo.name }),
                replace: true,
                resetScroll: false,
              });
            }}
            onSync={onSync}
          />
        ))}
        {isCut && index === visible.length - 1 && (
          <button
            type="button"
            className="flex w-full cursor-pointer items-center gap-1.5 border-t border-divider px-4 py-2.5 text-left text-base text-primary hover:bg-muted [&>svg]:size-3.5"
            onClick={() => {
              setIsAllShown(true);
            }}
          >
            Show the {plural(repos.length - shown.length, "other repository", "other repositories")}
            <ArrowIcon />
          </button>
        )}
      </Card>
    </Fragment>
  ));
}

/** The id of the row of `repo`: "Start with" scrolls to it. */
export function startRowId(repo: Pick<Repo, "id">): string {
  return `repo-${repo.id}`;
}

function StartRow({
  org,
  repo,
  isOpen,
  onToggle,
  onSync,
}: {
  org: string;
  repo: Repo;
  isOpen: boolean;
  onToggle: () => void;
  onSync: () => void;
}) {
  const { state } = repo.setup;
  const isBlocked = isSetupBlocked(state);
  const detailId = `start-${repo.id}`;

  return (
    <div
      id={startRowId(repo)}
      className={cn("border-b border-divider last:border-b-0", isOpen && "bg-muted")}
    >
      <button
        type="button"
        className="grid min-h-11 w-full cursor-pointer grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-4 px-4 py-1.5 text-left text-md hover:bg-muted max-md:grid-cols-[minmax(0,1fr)_auto]"
        aria-expanded={isOpen}
        aria-controls={detailId}
        onClick={onToggle}
      >
        <span className="flex min-w-0 items-center">
          <span className="text-muted-foreground">{repo.owner}/</span>
          <b className="truncate font-medium">{repo.name}</b>
          <code className="ml-2.5">{repo.default_branch}</code>
        </span>
        <span className="max-md:col-start-1 max-md:row-start-2">
          <SetupStateBadge state={state} />
        </span>
        <span
          className={cn(
            buttonVariants({ variant: isBlocked || isOpen ? "default" : "primary" }),
            "max-md:row-span-2",
          )}
        >
          {isOpen ? "Close" : ACTIONS[state]}
        </span>
      </button>
      {isOpen && (
        <div id={detailId} className="grid gap-2.5 px-4 pt-1 pb-4">
          <Suspense fallback={<StartDetailSkeleton />}>
            <StartDetail org={org} name={repo.name} onSync={onSync} />
          </Suspense>
        </div>
      )}
    </div>
  );
}
