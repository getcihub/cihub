import { useEffect, useState } from "react";

import type { OrgPr, OrgQueue } from "../client/types.gen";
import styles from "./Mine.module.css";
import { whereOf } from "./reads";

const SHOWN_MS = 8_000;

function whereEach(prs: OrgPr[], queues: OrgQueue[]): [string, string][] {
  return prs.map((pr) => {
    const queue = queues.find((entry) => entry.repo === pr.repo && entry.queue.name === pr.queue)?.queue;
    return [pr.item.id, whereOf(pr, queue?.state === "paused", queue?.target_branch ?? pr.queue)];
  });
}

/** Tells the person, for a moment, that one of their PRs moved: "#124 Fix the login redirect: testing". */
export function MoveNotice({ prs, queues }: { prs: OrgPr[]; queues: OrgQueue[] }) {
  const where = whereEach(prs, queues);
  const shown = JSON.stringify(where);
  const [seen, setSeen] = useState(shown);
  const [notice, setNotice] = useState<string | null>(null);

  // The where of the last render: a PR whose where changed since then moved.
  if (seen !== shown) {
    const before = new Map(JSON.parse(seen) as [string, string][]);
    const moved = prs.find((pr, index) => {
      const was = before.get(pr.item.id);
      return was !== undefined && was !== where[index]?.[1];
    });
    setSeen(shown);
    if (moved) {
      const title = moved.item.title ?? "";
      setNotice(`#${String(moved.item.pr_number)} ${title}: ${where[prs.indexOf(moved)]?.[1] ?? ""}`);
    }
  }

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
      {notice && <p className={styles.toast}>{notice}</p>}
    </div>
  );
}
