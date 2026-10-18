import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Queue, Repo, SetupState } from "@/client/types.gen";
import { COMMAND } from "@/components/queue/copy-command";
import { QueueStateBadge } from "@/components/queue/queue-state-badge";
import { SetupStateBadge } from "@/components/setup/setup-state-badge";
import { buttonVariants } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { inQueueCount, testingCount } from "@/domain/queue/counts";
import { timeAgo } from "@/lib/time";

/**
 * The repositories of an organization, by name, each with the verdict of its setup and its queues below it; a
 * repository without a queue says how the first one starts.
 */
export function RepoList({
  org,
  repos,
  queues,
  now,
  empty,
}: {
  org: string;
  repos: Repo[];
  queues: Queue[];
  now: number;
  /** What the list says without a repository. */
  empty: string;
}) {
  return (
    <Card className="p-0">
      {repos.map((repo) => (
        <RepoRow
          key={repo.id}
          org={org}
          repo={repo}
          queues={queues.filter((queue) => queue.repo === repo.name)}
          now={now}
        />
      ))}
      {repos.length === 0 && <p className="px-4 py-6 text-md text-muted-foreground">{empty}</p>}
    </Card>
  );
}

const START = {
  action: "Start",
  hint: (branch: string) => (
    <>
      comment <code>{COMMAND}</code> on a pull request to <code>{branch}</code>.
    </>
  ),
};
const FIX = {
  action: "Fix",
  hint: () => (
    <>
      fix its setup, then comment <code>{COMMAND}</code> on a pull request.
    </>
  ),
};

/** What a repository without a queue leads to, by the verdict of its setup. */
const FIRST_QUEUE: Record<SetupState, { action: string; hint: (branch: string) => ReactNode }> = {
  checking: { action: "Open", hint: () => "CIHub reads the rules and the configuration of the repository." },
  // The server started a queue that the list of the queues does not hold yet.
  started: { action: "Open", hint: () => "its queue comes in a moment." },
  ready: START,
  ready_with_warnings: START,
  cannot_merge: FIX,
  invalid_config: FIX,
};

function RepoRow({ org, repo, queues, now }: { org: string; repo: Repo; queues: Queue[]; now: number }) {
  const { state } = repo.setup;
  const firstQueue = FIRST_QUEUE[state];

  return (
    <div className="border-b border-divider px-4 py-2.5 last:border-b-0">
      <div className="grid grid-cols-[1fr_auto_auto] items-center gap-4">
        <Link
          className="flex min-w-0 items-center gap-1 text-md"
          to="/orgs/$org/repos/$repo"
          params={{ org, repo: repo.name }}
        >
          <span className="text-muted-foreground">{repo.namespace}/</span>
          <b className="font-medium">{repo.name}</b>
        </Link>
        {/* Once a queue runs, its lines say how it goes. */}
        {queues.length === 0 && <SetupStateBadge state={state} />}
        {queues.length === 0 && (
          <Link className={buttonVariants()} to="/orgs/$org/repos/$repo" params={{ org, repo: repo.name }}>
            {firstQueue.action}
          </Link>
        )}
      </div>
      {queues.length > 0 ? (
        <ul className="mt-1.5 flex flex-col">
          {queues.map((queue) => (
            <QueueLine key={queue.id} org={org} queue={queue} now={now} />
          ))}
        </ul>
      ) : (
        <p className="mt-1 text-md text-muted-foreground">
          No queue yet: {firstQueue.hint(repo.default_branch)}
        </p>
      )}
    </div>
  );
}

/** A queue of a repository on one line: its branch, its state and its counters; its page on a click. */
function QueueLine({ org, queue, now }: { org: string; queue: Queue; now: number }) {
  const { counts } = queue;

  return (
    <li>
      <Link
        className="grid grid-cols-[minmax(8rem,auto)_auto_1fr] items-center gap-4 rounded-sm py-1 pl-3 text-md text-inherit hover:bg-muted"
        to="/orgs/$org/repos/$repo/queues/$queue"
        params={{ org, repo: queue.repo, queue: queue.target_branch }}
      >
        <span className="font-mono">{queue.target_branch}</span>
        <QueueStateBadge queue={queue} />
        <span className="text-muted-foreground">
          {inQueueCount(counts)} in the queue · {testingCount(counts)} testing · {queue.merged_last_24h}{" "}
          merged in 24 h
          {counts.pending_failure > 0 && (
            <>
              {" · "}
              <span className="text-warning">{counts.pending_failure} with failed tests</span>
            </>
          )}
          {" · "}
          {queue.last_merged_at ? `last merge ${timeAgo(queue.last_merged_at, now)}` : "no merge yet"}
        </span>
      </Link>
    </li>
  );
}
