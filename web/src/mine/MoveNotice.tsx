import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import type { Pr, Queue } from "../client/types.gen";
import ui from "../ui/ui.module.css";
import styles from "./Mine.module.css";
import { minePrsOf, minePrsReads, whereOf } from "./reads";

const SHOWN_MS = 8_000;

function whereEach(prs: Pr[], queues: Queue[]): Map<string, [string, string]> {
  return new Map(
    prs.map((pr) => {
      const queue = queues.find((entry) => entry.repo === pr.repo && entry.name === pr.queue);
      const where = whereOf(pr, queue?.state === "paused", queue?.target_branch ?? pr.queue);
      return [pr.id, [`#${String(pr.number)} ${pr.title ?? ""}`, where]];
    }),
  );
}

/**
 * Tells the person, for a moment, which of their PRs moved: "#124 Fix the login redirect: testing". It compares
 * the reads of their PRs in the cache, the last one with the new one.
 */
export function MoveNotice({ org, queues }: { org: string; queues: Queue[] }) {
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    const reads = minePrsReads(org);
    const keys = [JSON.stringify(reads.live.queryKey), JSON.stringify(reads.merged.queryKey)];
    const mine = () =>
      minePrsOf(
        queryClient.getQueryData(reads.live.queryKey)?.prs,
        queryClient.getQueryData(reads.merged.queryKey)?.prs,
      );
    let last = whereEach(mine(), queues);

    return queryClient.getQueryCache().subscribe((event) => {
      if (event.type !== "updated" || !keys.includes(JSON.stringify(event.query.queryKey))) {
        return;
      }
      const next = whereEach(mine(), queues);
      const moved = [...next].filter(([id, [, where]]) => {
        const before = last.get(id);
        return before !== undefined && before[1] !== where;
      });
      last = next;
      if (moved.length > 0) {
        setNotice(moved.map(([, [pr, where]]) => `${pr}: ${where}`).join(" · "));
      }
    });
  }, [org, queues, queryClient]);

  useEffect(() => {
    if (!notice) {
      return;
    }
    const timer = setTimeout(() => {
      setNotice(null);
    }, SHOWN_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [notice]);

  return (
    <div className={styles.toasts} role="status">
      {notice && (
        <p className={styles.toast}>
          {notice}{" "}
          <button
            type="button"
            className={ui.button}
            onClick={() => {
              setNotice(null);
            }}
          >
            Dismiss
          </button>
        </p>
      )}
    </div>
  );
}
