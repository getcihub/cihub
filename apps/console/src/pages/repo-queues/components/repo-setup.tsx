import { type ReactNode, useState } from "react";

import type { ConfigFile } from "@/client/types.gen";
import { COMMAND } from "@/components/queue/copy-command";
import { CheckAgainButton } from "@/components/setup/check-again-button";
import { ConfigText } from "@/components/setup/config-text";
import { SetupRules } from "@/components/setup/setup-rules";
import { SetupStateBadge } from "@/components/setup/setup-state-badge";
import { Button } from "@cihub/ui/components/button";
import { Card, CardHeader, CardHint, CardTitle } from "@cihub/ui/components/card";
import { Prose } from "@cihub/ui/components/prose";
import { cn } from "@cihub/ui/lib/cn";
import type { RepoSetupRead } from "@/hooks/use-repo-setup";
import { timeAgo } from "@/lib/time";

/**
 * The setup of a repository: the rules of its default branch and its configuration file. An admin of the
 * repository asks for a read again with "Check again". The setup folds into its head when every point passes, and
 * opens by itself while a point fails or CIHub checks it; a click flips it, until the verdict changes.
 */
export function RepoSetup({
  repo,
  isChecking,
  hasGivenUp,
  checkAgain,
  now,
}: RepoSetupRead & { now: number }) {
  const { setup, default_branch: branch } = repo;
  const hasFailure =
    setup.target_branch_missing ||
    setup.config?.state === "invalid" ||
    setup.rules.some((rule) => !rule.passed);
  const isOpenByDefault = isChecking || hasFailure;
  // The choice of the person, with the verdict it answered: a new verdict opens or folds the card by itself.
  const [choice, setChoice] = useState<{ isOpen: boolean; byDefault: boolean } | null>(null);
  const isOpen = choice?.byDefault === isOpenByDefault ? choice.isOpen : isOpenByDefault;

  return (
    <Card className="p-0">
      <CardHeader className={cn(!isOpen && "border-b-0")}>
        <CardTitle>
          Setup of <code>{branch}</code>
        </CardTitle>
        <SetupStateBadge state={setup.state} />
        <CardHint className="flex items-center gap-2">
          {hasGivenUp
            ? "Not read yet: reload the page in a moment"
            : isChecking
              ? "Checking now"
              : setup.checked_at && `Checked ${timeAgo(setup.checked_at, now)}`}
          {checkAgain && (
            <CheckAgainButton checkAgain={checkAgain} isChecking={isChecking} hasGivenUp={hasGivenUp} />
          )}
          <Button
            aria-expanded={isOpen}
            onClick={() => {
              setChoice({ isOpen: !isOpen, byDefault: isOpenByDefault });
            }}
          >
            {isOpen ? "Hide" : "Show"}
          </Button>
        </CardHint>
      </CardHeader>
      {isOpen && (
        <>
          <Section
            title={
              <>
                1 · The rules of <code>{branch}</code>
              </>
            }
          >
            <SetupRules repo={repo} isChecking={isChecking} />
          </Section>
          <Section title="2 · The configuration" tag="optional">
            <Configuration branch={branch} config={setup.config ?? null} />
          </Section>
        </>
      )}
    </Card>
  );
}

function Section({ title, tag, children }: { title: ReactNode; tag?: string; children: ReactNode }) {
  return (
    <section className="border-t border-divider px-4 py-3.5 first-of-type:border-t-0">
      <h3 className="mb-2 text-md font-medium text-secondary-foreground">
        {title}
        {tag && <span className="ml-2 text-sm font-normal text-muted-foreground">{tag}</span>}
      </h3>
      {children}
    </section>
  );
}

/** The configuration file at the head of the default branch: none and the defaults, its entries, or its error. */
function Configuration({ branch, config }: { branch: string; config: ConfigFile | null }) {
  if (!config) {
    return <p className="text-md text-muted-foreground">Reading the configuration…</p>;
  }

  if (config.state === "invalid") {
    return (
      <>
        <p className="mb-2 text-md text-secondary-foreground">
          <code>{config.path ?? ".cihub.yaml"}</code> is not valid: <Prose text={config.error ?? ""} />
        </p>
        {config.text && <ConfigText text={config.text} errorLine={config.line ?? null} />}
        <p className="mt-2 text-md text-muted-foreground">
          A <code>{COMMAND}</code> still creates the queue of <code>{branch}</code>, but each new test run
          fails until the file is fixed on <code>{branch}</code>.
        </p>
      </>
    );
  }

  const { settings } = config;
  return (
    <>
      <p className="mb-2 text-md text-secondary-foreground">
        {config.state === "valid" ? (
          <>
            <code>{config.path}</code> is valid. It names {config.entries.length}{" "}
            {config.entries.length === 1 ? "queue" : "queues"}:{" "}
            {config.entries.map((entry, index) => (
              <span key={entry}>
                {index > 0 && ", "}
                <code>{entry}</code>
              </span>
            ))}
            . The queue of <code>{branch}</code> takes these settings:
          </>
        ) : (
          <>
            No <code>.cihub.yaml</code>. The first <code>{COMMAND}</code> creates the queue of{" "}
            <code>{branch}</code>, with the default settings:
          </>
        )}
      </p>
      <dl className="grid max-w-140 grid-cols-[8rem_1fr] gap-x-4 gap-y-1 text-md">
        <dt className="text-muted-foreground">Strategy</dt>
        <dd>{config.strategy}</dd>
        <dt className="text-muted-foreground">Merge method</dt>
        <dd>{settings.merge_method}</dd>
        <dt className="text-muted-foreground">Batches</dt>
        <dd>
          {settings.batching_enabled
            ? `up to ${String(settings.batching_size)}, after ${String(settings.batching_max_wait_minutes)} minutes at most`
            : "none"}
        </dd>
        <dt className="text-muted-foreground">Failure budget</dt>
        <dd>
          {settings.failure_budget_retries} {settings.failure_budget_retries === 1 ? "retry" : "retries"}
        </dd>
        {settings.enqueue_label && (
          <>
            <dt className="text-muted-foreground">Label</dt>
            <dd>
              <code>{settings.enqueue_label}</code> submits a pull request
            </dd>
          </>
        )}
      </dl>
    </>
  );
}
