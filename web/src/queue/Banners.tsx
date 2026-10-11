import type { ReactNode } from "react";

import type { ConfigCheck, QueueDetail } from "../client/types.gen";
import { timeAgo } from "../time";
import { DrainIcon, PauseIcon, WarningIcon } from "../ui/icons";
import styles from "./queue.module.css";

type Tone = "red" | "amber" | "blue";

function Banner({
  tone,
  icon,
  title,
  children,
}: {
  tone: Tone;
  icon: ReactNode;
  title: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className={`${styles.banner} ${styles[tone]}`}>
      {icon}
      <div>
        <b>{title}</b>
        {children && <p>{children}</p>}
      </div>
    </div>
  );
}

/** What stops or slows the queue: a lost access, a file that is not valid, a pause, a drain, the rules of the SCM. */
export function Banners({
  detail,
  check,
  org,
  repo,
  now,
}: {
  detail: QueueDetail;
  check: ConfigCheck;
  org: string;
  repo: string;
  now: number;
}) {
  const { queue } = detail;
  const isAccessLost = queue.access === "suspended";
  const when = timeAgo(queue.state_changed_at, now);

  return (
    <>
      {isAccessLost && (
        <Banner tone="red" icon={<WarningIcon />} title={`CIHub lost its access to ${org}/${repo}.`}>
          The queue keeps its pull requests and their order, and merges nothing until the access returns.
        </Banner>
      )}
      {queue.config_error && (
        <Banner
          tone="red"
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
          tone="amber"
          icon={<PauseIcon />}
          title={queue.state_changed_by ? `Paused by @${queue.state_changed_by} ${when}.` : `Paused ${when}.`}
        >
          The queue starts no test and merges nothing. It keeps its order: a resume starts where it stopped.
        </Banner>
      )}
      {queue.state === "draining" && (
        <Banner tone="blue" icon={<DrainIcon />} title="Draining.">
          The queue accepts no new pull request and lets the ones in it merge.
        </Banner>
      )}
      {check.findings.map((finding) => (
        <Banner key={finding.code} tone="amber" icon={<WarningIcon />} title={finding.message} />
      ))}
    </>
  );
}
