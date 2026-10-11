import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import type { OrgPrList, OrgQueue } from "../client/types.gen";
import ui from "../ui/ui.module.css";
import styles from "./Mine.module.css";
import { minePrsOptions, whereOf } from "./reads";

const SHOWN_MS = 8_000;

function whereEach(prs: OrgPrList | undefined, queues: OrgQueue[]): Map<string, [string, string]> {
  return new Map(
    (prs?.prs ?? []).map((pr) => {
      const queue = queues.find((entry) => entry.repo === pr.repo && entry.queue.name === pr.queue)?.queue;
      const where = whereOf(pr, queue?.state === "paused", queue?.target_branch ?? pr.queue);
      return [pr.item.id, [`#${String(pr.item.pr_number)} ${pr.item.title ?? ""}`, where]];
    }),
  );
}

/**
 * Tells the person, for a moment, which of their PRs moved: "#124 Fix the login redirect: testing". It compares
 * the reads of their PRs in the cache, the last one with the new one.
 */
export function MoveNotice({ org, login, queues }: { org: string; login: string; queues: OrgQueue[] }) {
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    const key = JSON.stringify(minePrsOptions(org, login).queryKey);
    let last = whereEach(queryClient.getQueryData(minePrsOptions(org, login).queryKey), queues);

    return queryClient.getQueryCache().subscribe((event) => {
      if (event.type !== "updated" || JSON.stringify(event.query.queryKey) !== key) {
        return;
      }
      const next = whereEach(event.query.state.data as OrgPrList | undefined, queues);
      const moved = [...next].filter(([id, [, where]]) => {
        const before = last.get(id);
        return before !== undefined && before[1] !== where;
      });
      last = next;
      if (moved.length > 0) {
        setNotice(moved.map(([, [pr, where]]) => `${pr}: ${where}`).join(" · "));
      }
    });
  }, [org, login, queues, queryClient]);

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
