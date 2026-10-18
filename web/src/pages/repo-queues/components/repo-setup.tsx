import type { ReactNode } from "react";

import type { ConfigFile, RepoDetail, RuleCheck } from "@/client/types.gen";
import { CheckIcon } from "@/components/icons/check-icon";
import { ErrorIcon } from "@/components/icons/error-icon";
import { WarningIcon } from "@/components/icons/warning-icon";
import { COMMAND, CopyCommand } from "@/components/queue/copy-command";
import { SetupStateBadge } from "@/components/setup/setup-state-badge";
import { Alert, AlertContent, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardHint, CardTitle } from "@/components/ui/card";
import { Prose } from "@/components/ui/prose";
import { isSetupRead } from "@/domain/setup/queries";
import { cn } from "@/lib/cn";
import { timeAgo } from "@/lib/time";

/**
 * The setup of a repository: the rules of its default branch, its configuration file, and, before its first
 * queue, how the queue starts. An admin of the repository asks for a read again with "Check again".
 */
export function RepoSetup({
  repo,
  hasQueue,
  now,
  hasGivenUp,
  checkAgain,
}: {
  repo: RepoDetail;
  hasQueue: boolean;
  now: number;
  /** The read takes longer than the page waits: it says so, and reads no more. */
  hasGivenUp: boolean;
  checkAgain: { run: () => void; isPending: boolean } | null;
}) {
  const { setup, default_branch: branch } = repo;
  const isChecking = !isSetupRead(setup);

  return (
    <Card className="mb-5 p-0">
      <CardHeader>
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
            <Button
              isPending={checkAgain.isPending || (isChecking && !hasGivenUp)}
              pendingText="Checking"
              onClick={checkAgain.run}
            >
              Check again
            </Button>
          )}
        </CardHint>
      </CardHeader>
      <Section
        title={
          <>
            1 · The rules of <code>{branch}</code>
          </>
        }
      >
        <Rules repo={repo} isChecking={isChecking} />
      </Section>
      <Section title="2 · The configuration" tag="optional">
        <Configuration branch={branch} config={setup.config ?? null} />
      </Section>
      {!hasQueue && (
        <Section title="3 · Start the queue">
          <p className="mb-2 text-md text-secondary-foreground">
            Comment on an approved pull request that targets <code>{branch}</code>:
          </p>
          <CopyCommand />
          <p className="mt-2 text-md text-muted-foreground">
            The queue appears in CIHub at this first <code>{COMMAND}</code>.
          </p>
        </Section>
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

/** The four rules that a queue needs, as the server read them; the link to change them on the SCM. */
function Rules({ repo, isChecking }: { repo: RepoDetail; isChecking: boolean }) {
  const { setup } = repo;

  if (setup.target_branch_missing) {
    return (
      <Alert variant="destructive" role="status">
        <ErrorIcon />
        <AlertContent>
          <AlertTitle>
            The branch <code>{repo.default_branch}</code> does not exist: no queue can merge into it.
          </AlertTitle>
        </AlertContent>
      </Alert>
    );
  }
  if (setup.rules.length === 0) {
    return (
      <p className="text-md text-muted-foreground">{isChecking ? "Reading the rules…" : "Not read yet."}</p>
    );
  }

  return (
    <>
      <ul className="flex flex-col gap-2">
        {setup.rules.map((rule) => (
          <Rule key={rule.code} rule={rule} />
        ))}
      </ul>
      {setup.rules.some((rule) => !rule.passed) && setup.rules_url && (
        <a className="mt-3 inline-block text-md" href={setup.rules_url}>
          Open the rules of {repo.name} on GitHub →
        </a>
      )}
    </>
  );
}

function Rule({ rule }: { rule: RuleCheck }) {
  const tone = rule.passed ? "text-success" : rule.blocks_merge ? "text-destructive" : "text-warning";
  const icon = rule.passed ? <CheckIcon /> : rule.blocks_merge ? <ErrorIcon /> : <WarningIcon />;

  return (
    <li className="text-md">
      <div className="flex items-center gap-2">
        <span className={cn("flex-none [&>svg]:size-4", tone)}>{icon}</span>
        <span>
          <Prose text={rule.title} />
        </span>
        {!rule.passed && (
          <span className="text-sm text-muted-foreground">
            {rule.blocks_merge ? "blocks every merge" : "warning"}
          </span>
        )}
      </div>
      {rule.message && (
        <p className="ml-6 text-md text-secondary-foreground">
          <Prose text={rule.message} />
        </p>
      )}
    </li>
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
        {config.text && <FileText text={config.text} errorLine={config.line ?? null} />}
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

/** The text of a file that is not valid, with the line of its error marked. */
function FileText({ text, errorLine }: { text: string; errorLine: number | null }) {
  return (
    <pre className="overflow-x-auto rounded-sm border border-border bg-muted py-1.5 font-mono text-md">
      {text.split("\n").map((line, index) => {
        const number = index + 1;
        return (
          <span
            key={number}
            className={cn("block px-3", number === errorLine && "bg-destructive-muted text-destructive")}
          >
            <i className="mr-3 inline-block w-6 text-right text-muted-foreground not-italic select-none">
              {number}
            </i>
            {line}
          </span>
        );
      })}
    </pre>
  );
}
