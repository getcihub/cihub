import type { ReactNode } from "react";

import type { RepoDetail } from "@/client/types.gen";
import { ArrowIcon } from "@/components/icons/arrow-icon";
import { WarningIcon } from "@/components/icons/warning-icon";
import { CopyCommand, MergeCommand } from "@/components/queue/copy-command";
import { CheckAgainButton } from "@/components/setup/check-again-button";
import { FirstCommandWait } from "@/components/setup/first-command-wait";
import { buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { EmptyState } from "@/components/ui/empty-state";
import { FailureIllustration, QueueIllustration } from "@/components/ui/illustrations";
import { Prose } from "@/components/ui/prose";
import { isSetupBlocked } from "@/domain/setup/queries";
import type { RepoSetupRead } from "@/hooks/use-repo-setup";
import { plural, shortSha } from "@/lib/text";

/**
 * A repository without a queue: the one thing to do, and what CIHub waits for. A blocked setup says what to fix
 * first; the setup below says the details.
 */
export function RepoStart({ repo, isChecking, hasGivenUp, checkAgain }: RepoSetupRead) {
  const { setup, default_branch: branch } = repo;

  if (isChecking) {
    return (
      <Card>
        <EmptyState illustration={<QueueIllustration />} title="Checking the setup">
          CIHub reads the rules of <code>{branch}</code> and its configuration.
        </EmptyState>
      </Card>
    );
  }
  if (isSetupBlocked(setup.state)) {
    return (
      <Card>
        <EmptyState
          illustration={<FailureIllustration />}
          title={
            <>
              <code>{branch}</code> cannot have a queue yet
            </>
          }
          actions={
            <>
              {setup.state === "cannot_merge" && !setup.target_branch_missing && setup.rules_url && (
                <a className={buttonVariants({ variant: "primary" })} href={setup.rules_url}>
                  Open the rules on GitHub
                  <ArrowIcon />
                </a>
              )}
              {checkAgain && (
                <CheckAgainButton checkAgain={checkAgain} isChecking={isChecking} hasGivenUp={hasGivenUp} />
              )}
            </>
          }
          foot="The setup below says what to change."
        >
          {blockOf(repo)}: the queue could not merge. Fix it, check again, then comment <MergeCommand /> on a
          pull request.
        </EmptyState>
      </Card>
    );
  }

  const failed = setup.rules.filter((rule) => !rule.passed);
  const { config } = setup;
  return (
    <Card>
      <EmptyState
        illustration={<QueueIllustration />}
        title={
          <>
            No queue on <code>{branch}</code> yet
          </>
        }
        actions={
          <div className="grid justify-items-center gap-2">
            <CopyCommand />
            <FirstCommandWait className="rounded-md border border-border bg-muted px-3.5 py-2">
              This page moves on by itself: the queue appears here.
            </FirstCommandWait>
            {failed[0] && (
              <p className="flex max-w-130 items-start gap-2 text-left text-md text-secondary-foreground">
                <WarningIcon className="mt-0.5 size-3.5 flex-none text-warning" />
                <span>
                  The queue can start, with{" "}
                  {failed.length > 1 ? `${String(failed.length)} warnings` : "a warning"}.{" "}
                  <Prose text={failed[0].title} />. See the setup below.
                </span>
              </p>
            )}
          </div>
        }
        foot={
          config && (
            <>
              Settings:{" "}
              {config.state === "valid" ? (
                <code>
                  {config.path} @ {config.sha && shortSha(config.sha)}
                </code>
              ) : (
                <>
                  the defaults: {config.strategy}, {config.settings.merge_method} merge
                </>
              )}
            </>
          )
        }
      >
        Comment on an approved pull request to <code>{branch}</code>: CIHub creates the queue and tests the
        pull request.
      </EmptyState>
    </Card>
  );
}

/** What blocks the first queue: the file, the branch, or how many rules to fix. */
function blockOf({ setup, default_branch: branch }: RepoDetail): ReactNode {
  if (setup.state === "invalid_config") {
    return (
      <>
        <code>{setup.config?.path ?? ".cihub.yaml"}</code> is not valid
      </>
    );
  }
  if (setup.target_branch_missing) {
    return (
      <>
        <code>{branch}</code> has no commit yet
      </>
    );
  }
  const blocking = setup.rules.filter((rule) => !rule.passed && rule.blocks_merge).length;

  return `${plural(blocking, "branch rule", "branch rules")} to fix`;
}
