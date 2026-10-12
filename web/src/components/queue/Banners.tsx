import type { ComponentProps, ReactNode } from "react";

import type { Queue } from "@/client/types.gen";
import { timeAgo } from "@/lib/time";
import { Alert, AlertContent, AlertDescription, AlertTitle } from "@/ui/alert";
import { DrainIcon, PauseIcon, WarningIcon } from "@/ui/icons";

function Banner({
  variant,
  icon,
  title,
  children,
}: {
  variant: ComponentProps<typeof Alert>["variant"];
  icon: ReactNode;
  title: ReactNode;
  children?: ReactNode;
}) {
  return (
    // A banner describes the queue as the page opens: it informs, it does not interrupt.
    <Alert variant={variant} role="status">
      {icon}
      <AlertContent>
        <AlertTitle>{title}</AlertTitle>
        {children && <AlertDescription>{children}</AlertDescription>}
      </AlertContent>
    </Alert>
  );
}

/** What stops or slows the queue: a lost access, a file that is not valid, a pause, a drain, the rules of the SCM. */
export function Banners({ queue, org, repo, now }: { queue: Queue; org: string; repo: string; now: number }) {
  const isAccessLost = queue.access === "suspended";
  const when = timeAgo(queue.state_changed_at, now);

  return (
    <>
      {isAccessLost && (
        <Banner
          variant="destructive"
          icon={<WarningIcon />}
          title={`CIHub lost its access to ${org}/${repo}.`}
        >
          The queue keeps its pull requests and their order, and merges nothing until the access returns.
        </Banner>
      )}
      {queue.config_error && (
        <Banner
          variant="destructive"
          icon={<WarningIcon />}
          title={
            <>
              The <code>.cihub.yaml</code> at the head of <code>{queue.target_branch}</code> is not valid.
            </>
          }
        >
          The queue keeps the settings of its last valid file: {queue.config_error}
        </Banner>
      )}
      {queue.state === "paused" && !isAccessLost && (
        <Banner
          variant="warning"
          icon={<PauseIcon />}
          title={queue.state_changed_by ? `Paused by @${queue.state_changed_by} ${when}.` : `Paused ${when}.`}
        >
          The queue starts no test and merges nothing. It keeps its order: a resume starts where it stopped.
        </Banner>
      )}
      {queue.state === "draining" && (
        <Banner variant="info" icon={<DrainIcon />} title="Draining.">
          The queue accepts no new pull request and lets the ones in it merge.
        </Banner>
      )}
      {queue.config_check.findings.map((finding) => (
        <Banner key={finding.code} variant="warning" icon={<WarningIcon />} title={finding.message} />
      ))}
    </>
  );
}
