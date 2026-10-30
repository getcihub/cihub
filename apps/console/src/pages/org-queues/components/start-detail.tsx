import { Link } from "@tanstack/react-router";
import { type ReactNode } from "react";

import type { ConfigFile, RepoDetail, SetupState } from "@/client/types.gen";
import { CheckIcon } from "@cihub/icons/check";
import { ErrorIcon } from "@cihub/icons/error";
import { WarningIcon } from "@cihub/icons/warning";
import { Prose } from "@cihub/ui/components/prose";
import { Skeleton } from "@cihub/ui/components/skeleton";
import { Spinner } from "@cihub/ui/components/spinner";
import { cn } from "@cihub/ui/lib/cn";
import { CopyCommand } from "@/components/queue/copy-command";
import { COMMAND } from "@/domain/queue/words";
import { CheckAgainButton } from "@/components/setup/check-again-button";
import { ConfigText } from "@/components/setup/config-text";
import { FirstCommandWait } from "@/components/setup/first-command-wait";
import { SetupRules } from "@/components/setup/setup-rules";
import { isSetupBlocked } from "@/domain/setup/queries";
import { useRepoSetup } from "@/hooks/use-repo-setup";
import { plural, shortSha } from "@/lib/text";

/** The open row while its setup loads. */
export function StartDetailSkeleton() {
  return (
    <>
      <Skeleton className="w-[45%]" />
      <Skeleton className="w-[60%]" />
      <Skeleton className="h-16" />
    </>
  );
}

/** What the repository still needs: its rules, its settings, then the first command. */
export function StartDetail({ org, name, onSync }: { org: string; name: string; onSync: () => void }) {
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
