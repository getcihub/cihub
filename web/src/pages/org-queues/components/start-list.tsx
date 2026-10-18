import { Link, getRouteApi } from "@tanstack/react-router";
import { type ReactNode, Suspense, useState } from "react";

import type { ConfigFile, Repo, RepoDetail, SetupState } from "@/client/types.gen";
import { ArrowIcon } from "@/components/icons/arrow-icon";
import { CheckIcon } from "@/components/icons/check-icon";
import { ErrorIcon } from "@/components/icons/error-icon";
import { WarningIcon } from "@/components/icons/warning-icon";
import { COMMAND, CopyCommand } from "@/components/queue/copy-command";
import { ConfigText } from "@/components/setup/config-text";
import { SetupRules } from "@/components/setup/setup-rules";
import { SetupStateBadge } from "@/components/setup/setup-state-badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Prose } from "@/components/ui/prose";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { isSetupRead } from "@/domain/setup/queries";
import { useRepoSetup } from "@/hooks/use-repo-setup";
import { cn } from "@/lib/cn";
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

/**
 * The repositories without a queue: each row opens in place on what the repository still needs, in the order of
 * the work. The open row is in the address (`?open=`), so that a link keeps it.
 */
export function StartList({
  org,
  repos,
  open,
  isSearch,
  onSync,
}: {
  org: string;
  repos: Repo[];
  open: string | undefined;
  /** A search shows every match. */
  isSearch: boolean;
  /** A setup is read again: the list waits for it. */
  onSync: () => void;
}) {
  const navigate = route.useNavigate();
  const [isAllShown, setIsAllShown] = useState(false);
  const isCut = !isSearch && !isAllShown && repos.length > SHOWN_FIRST + 2;
  const shown = isCut ? repos.filter((repo, index) => index < SHOWN_FIRST || repo.name === open) : repos;

  return (
    <Card className="p-0">
      {shown.map((repo) => (
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
      {isCut && (
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
  );
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
  const isBlocked = state === "cannot_merge" || state === "invalid_config";
  const detailId = `start-${repo.id}`;

  return (
    <div className={cn("border-b border-divider last:border-b-0", isOpen && "bg-muted")}>
      <button
        type="button"
        className="grid min-h-11 w-full cursor-pointer grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-4 px-4 py-1.5 text-left text-md hover:bg-muted max-md:grid-cols-[minmax(0,1fr)_auto]"
        aria-expanded={isOpen}
        aria-controls={detailId}
        onClick={onToggle}
      >
        <span className="flex min-w-0 items-center">
          <span className="text-muted-foreground">{repo.namespace}/</span>
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
  const { repo, hasGivenUp, checkAgain } = useRepoSetup(org, name, onSync);
  const { setup, default_branch: branch } = repo;
  const isChecking = !isSetupRead(setup);
  const isBlocked = setup.state === "cannot_merge" || setup.state === "invalid_config";
  const check = checkAgain && (
    <Button
      isPending={checkAgain.isPending || (isChecking && !hasGivenUp)}
      pendingText="Checking"
      onClick={checkAgain.run}
    >
      Check again
    </Button>
  );

  return (
    <>
      <RulesLine repo={repo} isChecking={isChecking} check={check} />
      <SettingsLine org={org} repo={repo} config={setup.config ?? null} check={check} />
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
        ? "the branch is missing"
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
      <div className="flex items-center gap-2.5" role="status">
        <span className="mx-0.75 size-2.5 flex-none rounded-full bg-primary motion-safe:animate-beacon" />
        <div>
          <b className="block font-medium">Waiting for the first command</b>
          <span className="text-md text-muted-foreground">
            This row moves on by itself: the repository joins the ones with a queue.
          </span>
        </div>
      </div>
    </div>
  );
}
