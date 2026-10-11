import { Link } from "@tanstack/react-router";
import { useRef } from "react";

import type { Pr, Queue } from "../client/types.gen";
import { Prose } from "../queue/Prose";
import { useFlip } from "../ui/motion";
import styles from "./Mine.module.css";
import { Train } from "./Train";
import { etaOf, whereOf } from "./reads";

function PrTitle({ org, pr, className }: { org: string; pr: Pr; className: string }) {
  return (
    <Link
      className={className}
      to="/orgs/$org/repos/$repo/queues/$queue/prs/$number"
      params={{ org, repo: pr.repo, queue: pr.queue, number: pr.number }}
    >
      #{pr.number} {pr.title ?? ""}
    </Link>
  );
}

/** "Your pull requests" on the queues of an organization: where each one is, and when it should merge. */
export function MinePrs({ org, prs, queues }: { org: string; prs: Pr[]; queues: Queue[] }) {
  const list = useRef<HTMLUListElement>(null);
  useFlip(list);

  if (prs.length === 0) {
    return null;
  }

  return (
    <section className={styles.list} aria-labelledby="mine-title">
      <div className={styles.head}>
        <h2 id="mine-title">Your pull requests</h2>
        <span className={styles.hint}>Where each one is</span>
      </div>
      <ul ref={list}>
        {prs.map((pr) => {
          const queue = queues.find((entry) => entry.repo === pr.repo && entry.name === pr.queue);
          const branch = queue?.target_branch ?? pr.queue;
          const eta = etaOf(pr);

          return (
            <li key={pr.id} className={styles.row} data-flip={pr.id}>
              <div>
                <PrTitle org={org} pr={pr} className={styles.title} />
                <div className={styles.meta}>
                  {pr.repo} · {pr.queue}
                </div>
              </div>
              <Train pr={pr} branch={branch} />
              <div className={styles.status}>
                <b>{whereOf(pr, queue?.state === "paused", branch)}</b>
                {eta && <span>merges in {eta}</span>}
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/** The PRs of the person in the queue of the page, first on it. */
export function Spotlight({ org, prs, detail }: { org: string; prs: Pr[]; detail: Queue }) {
  const queue = detail;
  const branch = queue.target_branch;

  return (
    <>
      {prs.map((pr) => {
        const eta = etaOf(pr);
        const before = pr.ahead.length;

        return (
          <section key={pr.id} className={styles.spotlight} aria-label="Your pull request">
            <div className={styles.top}>
              <span className={styles.label}>Your pull request</span>
              <PrTitle org={org} pr={pr} className={styles.title} />
              {eta && (
                <span className={styles.eta}>
                  Merges in <b>{eta}</b>
                </span>
              )}
            </div>
            <div className={styles.middle}>
              <div className={styles.where}>
                <b>{whereOf(pr, queue.state === "paused", branch)}</b>
                {pr.state === "pending" && (
                  <span>
                    {before === 0
                      ? "next to be tested"
                      : `${String(before)} pull request${before > 1 ? "s" : ""} merge before it`}
                  </span>
                )}
              </div>
              <Train pr={pr} branch={branch} />
            </div>
            {pr.state !== "merged" && (
              <p className={styles.why}>
                <Prose text={pr.state_description} />
              </p>
            )}
          </section>
        );
      })}
    </>
  );
}
