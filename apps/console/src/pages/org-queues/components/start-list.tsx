import { Link, getRouteApi } from "@tanstack/react-router";
import { Fragment, type ReactNode, Suspense, useState } from "react";

import type { ConfigFile, Repo, RepoDetail, SetupState } from "@/client/types.gen";
import { ArrowIcon } from "@cihub/icons/arrow";
import { CheckIcon } from "@cihub/icons/check";
import { ErrorIcon } from "@cihub/icons/error";
import { WarningIcon } from "@cihub/icons/warning";
import { buttonVariants } from "@cihub/ui/components/button";
import { Card } from "@cihub/ui/components/card";
import { Prose } from "@cihub/ui/components/prose";
import { SectionCount, SectionHead, SectionTitle } from "@cihub/ui/components/section";
import { Skeleton } from "@cihub/ui/components/skeleton";
import { Spinner } from "@cihub/ui/components/spinner";
import { cn } from "@cihub/ui/lib/cn";
import { COMMAND, CopyCommand } from "@/components/queue/copy-command";
import { CheckAgainButton } from "@/components/setup/check-again-button";
import { ConfigText } from "@/components/setup/config-text";
import { FirstCommandWait } from "@/components/setup/first-command-wait";
import { SetupRules } from "@/components/setup/setup-rules";
import { SetupStateBadge } from "@/components/setup/setup-state-badge";
import { isSetupBlocked } from "@/domain/setup/queries";
import { useRepoSetup } from "@/hooks/use-repo-setup";
import { plural, shortSha } from "@/lib/text";

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

function StartDetailSkeleton() {
  return (
    <>
      <Skeleton className="w-[45%]" />
      <Skeleton className="w-[60%]" />
      <Skeleton className="h-16" />
    </>
  );
}

/** What the repository still needs: its rules, its settings, then the first command. */
function StartDetail({ org, name, onSync }: { org: string; name: string; onSync: () => void }) {
  const { repo, isChecking, hasGivenUp, checkAgain } = useRepoSetup(org, name, onSync);
  const { setup, default_branch: branch } = repo;
  const isBlocked = isSetupBlocked(setup.state);
  const check = checkAgain && (
    <CheckAgainButton checkAgain={checkAgain} isChecking={isChecking} hasGivenUp={hasGivenUp} />
  );
  // One "Check again" for the row: under the rules when they fail, else under the file.
  const isCheckUnderRules =
    !isChecking && (setup.target_branch_missing || setup.rules.some((rule) => !rule.passed));

  return (
    <>
      <RulesLine repo={repo} isChecking={isChecking} check={check} />
      <SettingsLine
        org={org}
        repo={repo}
        config={setup.config ?? null}
        check={isCheckUnderRules ? null : check}
      />
      {hasGivenUp && (
        <p className="text-md text-muted-foreground" role="status">
          Not read yet: reload the page in a moment.
        </p>
      )}
      {!isChecking && <FirstCommand branch={branch} state={setup.state} isBlocked={isBlocked} />}
      <Link className="text-md" to="/orgs/$org/repos/$repo" params={{ org, repo: name }}>
        The whole setup of {name} →
      </Link>
    </>
  );
}

type Tone = "ok" | "warn" | "bad" | "wait";

const TONES: Record<Tone, { icon: ReactNode; className: string }> = {
  ok: { icon: <CheckIcon />, className: "text-success" },
  warn: { icon: <WarningIcon />, className: "text-warning" },
  bad: { icon: <ErrorIcon />, className: "text-destructive" },
  wait: { icon: <Spinner />, className: "text-muted-foreground" },
};

/** A point of the setup on one line, in the color of its verdict, then what explains it below. */
function Point({ tone, children, detail }: { tone: Tone; children: ReactNode; detail?: ReactNode }) {
  const { icon, className } = TONES[tone];

  return (
    <div>
      <div className="flex items-start gap-2.5 text-base text-secondary-foreground">
        <span className={cn("mt-px flex-none [&>svg]:size-4", className)}>{icon}</span>
        <span>{children}</span>
      </div>
      {detail && <div className="mt-2 ml-6.5 grid justify-items-start gap-2">{detail}</div>}
    </div>
  );
}

function RulesLine({ repo, isChecking, check }: { repo: RepoDetail; isChecking: boolean; check: ReactNode }) {
  const { setup, default_branch: branch } = repo;
  const failed = setup.rules.filter((rule) => !rule.passed);
  const blocking = failed.filter((rule) => rule.blocks_merge).length;
  const label = <b className="font-medium text-foreground">Branch rules</b>;

  if (isChecking) {
    return <Point tone="wait">{label} · checking</Point>;
  }
  if (!setup.target_branch_missing && failed.length === 0) {
    return (
      <Point tone="ok">
        {label} · CIHub can merge on <code>{branch}</code>
      </Point>
    );
  }

  return (
    <Point
      tone={blocking > 0 || setup.target_branch_missing ? "bad" : "warn"}
      detail={
        <>
          <SetupRules repo={repo} isChecking={isChecking} />
          {check}
        </>
      }
    >
      {label} ·{" "}
      {setup.target_branch_missing
        ? "no commit yet"
        : blocking > 0
          ? `${plural(blocking, "point blocks", "points block")} every merge`
          : plural(failed.length, "warning", "warnings")}
    </Point>
  );
}

function SettingsLine({
  org,
  repo,
  config,
  check,
}: {
  org: string;
  repo: RepoDetail;
  config: ConfigFile | null;
  check: ReactNode;
}) {
  const label = <b className="font-medium text-foreground">Settings</b>;
  const settings = (
    <Link to="/orgs/$org/repos/$repo" params={{ org, repo: repo.name }}>
      See the settings
    </Link>
  );

  if (!config) {
    return <Point tone="wait">{label} · reading the configuration</Point>;
  }
  if (config.state === "invalid") {
    return (
      <Point
        tone="bad"
        detail={
          <>
            {config.text && <ConfigText text={config.text} errorLine={config.line ?? null} />}
            {check}
          </>
        }
      >
        {label} · <code>{config.path ?? ".cihub.yaml"}</code> is not valid:{" "}
        <Prose text={config.error ?? ""} />
      </Point>
    );
  }

  return (
    <Point tone="ok">
      {label} ·{" "}
      {config.state === "valid" ? (
        <code>
          {config.path} @ {config.sha && shortSha(config.sha)}
        </code>
      ) : (
        <>
          the defaults: {config.strategy}, {config.settings.merge_method} merge
        </>
      )}{" "}
      · {settings}
    </Point>
  );
}

/** How the first queue starts: the command to comment, once nothing blocks it. */
function FirstCommand({
  branch,
  state,
  isBlocked,
}: {
  branch: string;
  state: SetupState;
  isBlocked: boolean;
}) {
  if (state === "started") {
    return (
      <p className="rounded-md border border-border bg-card px-3.5 py-3 text-base">
        The first <code>{COMMAND}</code> started the queue of <code>{branch}</code>: it joins the repositories
        with a queue in a moment.
      </p>
    );
  }
  if (isBlocked) {
    return (
      <p className="rounded-md border border-dashed border-border px-3.5 py-3 text-base text-muted-foreground">
        Fix the points above, then comment <code>{COMMAND}</code> on a pull request to <code>{branch}</code>.
      </p>
    );
  }

  return (
    <div className="grid justify-items-start gap-2 rounded-md border border-border bg-card px-3.5 py-3 text-base">
      <span>
        Comment on an approved pull request to <code>{branch}</code>:
      </span>
      <CopyCommand />
      <FirstCommandWait>
        This row moves on by itself: the repository joins the ones with a queue.
      </FirstCommandWait>
    </div>
  );
}
