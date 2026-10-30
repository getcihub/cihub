import { Link } from "@tanstack/react-router";
import { useRef } from "react";

import type { Queue, Repo, SetupState } from "@/client/types.gen";
import { QueueStateBadge } from "@/components/queue/queue-state-badge";
import { Card } from "@cihub/ui/components/card";
import { useFlip } from "@cihub/ui/hooks/use-flip";
import { cn } from "@cihub/ui/lib/cn";
import { inQueueCount, testingCount } from "@/domain/queue/counts";
import { timeAgo } from "@/lib/time";

const HEAD_CELL = "px-4 py-2 text-left text-sm font-medium text-muted-foreground";
const CELL = "h-11 px-4";

/** A setup that concerns every queue of its repository, beside its name. */
const SETUP_DOTS: Partial<Record<SetupState, { label: string; className: string }>> = {
  ready_with_warnings: { label: "Setup: ready, with a warning", className: "bg-warning" },
  cannot_merge: { label: "Setup: cannot merge", className: "bg-destructive" },
  invalid_config: { label: "Setup: invalid .cihub.yaml", className: "bg-destructive" },
};

/**
 * The repositories that have a queue, in the order of `repos`: one row per queue, the name of the repository on
 * its first row only. A row opens its queue.
 */
export function QueueTable({
  org,
  repos,
  queues,
  now,
}: {
  org: string;
  repos: Repo[];
  queues: Queue[];
  now: number;
}) {
  const rows = useRef<HTMLTableSectionElement>(null);
  useFlip(rows);

  return (
    <Card className="overflow-x-auto p-0">
      <table className="w-full min-w-170">
        <thead>
          <tr className="border-b border-divider">
            <th className={HEAD_CELL}>Repository</th>
            <th className={HEAD_CELL}>Branch</th>
            <th className={HEAD_CELL}>State</th>
            <th className={cn(HEAD_CELL, "text-right")}>In the queue</th>
            <th className={cn(HEAD_CELL, "text-right")}>Testing</th>
            <th className={cn(HEAD_CELL, "text-right")}>Merged in 24 h</th>
            <th className={HEAD_CELL}>Last merge</th>
          </tr>
        </thead>
        <tbody ref={rows}>
          {repos.flatMap((repo) =>
            queues
              .filter((queue) => queue.repo === repo.name)
              .map((queue, index, ofRepo) => (
                <QueueRow
                  key={queue.id}
                  org={org}
                  repo={index === 0 ? repo : null}
                  queue={queue}
                  isLastOfRepo={index === ofRepo.length - 1}
                  now={now}
                />
              )),
          )}
        </tbody>
      </table>
    </Card>
  );
}

function QueueRow({
  org,
  repo,
  queue,
  isLastOfRepo,
  now,
}: {
  org: string;
  /** The repository on the first row of its queues, `null` on the others. */
  repo: Repo | null;
  queue: Queue;
  isLastOfRepo: boolean;
  now: number;
}) {
  const { counts } = queue;
  const dot = repo && SETUP_DOTS[repo.setup.state];

  return (
    // The link of the branch covers the row; the name of the repository stays a link of its own above it.
    <tr
      data-flip={queue.id}
      className={cn(
        "relative text-md hover:bg-muted",
        isLastOfRepo && "border-b border-divider last:border-b-0",
      )}
    >
      <td className={cn(CELL, "whitespace-nowrap")}>
        {repo && (
          <span className="relative z-10 inline-flex items-center gap-2">
            <Link
              className="text-inherit hover:text-primary"
              to="/orgs/$org/repos/$repo"
              params={{ org, repo: repo.name }}
            >
              <span className="text-muted-foreground">{repo.owner}/</span>
              <b className="font-medium">{repo.name}</b>
            </Link>
            {dot && (
              <span
                className={cn("size-1.5 rounded-full", dot.className)}
                title={dot.label}
                role="img"
                aria-label={dot.label}
              />
            )}
          </span>
        )}
      </td>
      <td className={CELL}>
        <Link
          className="font-mono text-inherit after:absolute after:inset-0"
          to="/orgs/$org/repos/$repo/queues/$queue"
          params={{ org, repo: queue.repo, queue: queue.target_branch }}
        >
          {queue.target_branch}
        </Link>
      </td>
      <td className={CELL}>
        <QueueStateBadge queue={queue} />
      </td>
      <td className={cn(CELL, "text-right tabular-nums")}>{inQueueCount(counts)}</td>
      <td className={cn(CELL, "text-right whitespace-nowrap tabular-nums")}>
        {testingCount(counts)}
        {counts.pending_failure > 0 && (
          <span className="ml-1.5 text-sm text-warning">{counts.pending_failure} failed</span>
        )}
      </td>
      <td className={cn(CELL, "text-right tabular-nums")}>{queue.merged_last_24h}</td>
      <td className={cn(CELL, "whitespace-nowrap text-muted-foreground")}>
        {queue.last_merged_at ? timeAgo(queue.last_merged_at, now) : "no merge yet"}
      </td>
    </tr>
  );
}
