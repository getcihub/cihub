import { useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";

import { meOptions } from "../client/@tanstack/react-query.gen";

import type { Item, Queue } from "../client/types.gen";
import { useState } from "react";

import { Spotlight } from "../mine/Mine";
import { minePrsOptions } from "../mine/reads";
import { useNow } from "../time";
import { EmptyState } from "../ui/EmptyState";
import { QueueStateBadge } from "../ui/StateBadge";
import ui from "../ui/ui.module.css";
import { QueueActions, SubmitPr } from "./Actions";
import { Banners } from "./Banners";
import { ItemTabs } from "./ItemTabs";
import { QueueGraph } from "./QueueGraph";
import styles from "./queue.module.css";
import { type QueuePath, type Tab, queueReads } from "./reads";
import { Settings, Stats } from "./Summary";

/** The order of the queue: by position, then the items not admitted yet, by arrival. */
function inQueueOrder(items: Item[]): Item[] {
  return [...items].sort(
    (a, b) =>
      (a.position ?? Number.MAX_SAFE_INTEGER) - (b.position ?? Number.MAX_SAFE_INTEGER) ||
      Date.parse(a.submitted_at) - Date.parse(b.submitted_at),
  );
}

function QuietQueue({ queue }: { queue: Queue }) {
  return (
    <div className={ui.card}>
      <EmptyState illustration={null} title="No pull request in the queue">
        Nothing waits in the queue of <code>{queue.target_branch}</code>, and nothing merged or failed in the
        last 24 hours. A pull request enters when someone comments <code>/cihub merge</code> on it.
      </EmptyState>
    </div>
  );
}

/** The page of a queue: what stops it, its counters and settings, its graph and its items. */
export function QueuePage({ path, tab }: { path: QueuePath; tab: Tab }) {
  const now = useNow();
  const reads = queueReads(path, tab, now);
  const [detail, graph, check, live] = useSuspenseQueries({
    queries: [reads.detail, reads.graph, reads.check, reads.live],
  });
  // The same read as `live` when the tab of the live items is open.
  const opened = useSuspenseQuery(reads.tab);
  const { data: me } = useSuspenseQuery(meOptions());
  const { data: mine } = useSuspenseQuery(minePrsOptions(path.org, me.login));
  const spotlit = mine.prs.filter((pr) => pr.repo === path.repo && pr.queue === path.queue);
  const role = me.orgs.find((org) => org.slug.toLowerCase() === path.org.toLowerCase())?.role;
  // A request is sent, not done: the queue decides at its next reconciliation.
  const [notice, setNotice] = useState<string | null>(null);
  const { queue } = detail.data;
  const items = inQueueOrder(live.data.items);
  // A position is an order, not a rank: the page shows 1, 2, 3 for the items admitted. The graph has every live
  // item, where the read of the items stops at 100.
  const ranks = new Map(
    graph.data.nodes
      .flatMap((node) => (node.position == null ? [] : [{ id: node.item_id, position: node.position }]))
      .sort((a, b) => a.position - b.position)
      .map((node, index) => [node.id, index + 1]),
  );
  // A tab that the address opens shows its items, even in a queue with nothing live.
  const isQuiet =
    tab === "live" &&
    items.length === 0 &&
    detail.data.merged_last_24h === 0 &&
    detail.data.failed_last_24h === 0;

  return (
    <main className={ui.page}>
      <div className={ui.head}>
        <div>
          <div className={ui.sub}>
            {path.org}/{path.repo}
          </div>
          <div className={styles.title}>
            <h1>{queue.name}</h1>
            <QueueStateBadge queue={queue} />
          </div>
        </div>
        <div className={styles.actions}>
          <QueueActions path={path} queue={queue} isAdmin={role === "admin"} onSent={setNotice} />
          <SubmitPr path={path} isDisabled={queue.access === "suspended"} onSent={setNotice} />
        </div>
      </div>
      {notice && (
        <p className={ui.notice} role="status">
          {notice}
        </p>
      )}
      <Spotlight org={path.org} prs={spotlit} detail={detail.data} />
      <Banners detail={detail.data} check={check.data} org={path.org} repo={path.repo} now={now} />
      {!isQuiet && <Stats detail={detail.data} now={now} />}
      <Settings detail={detail.data} />
      {isQuiet ? (
        <QuietQueue queue={queue} />
      ) : (
        <>
          <QueueGraph detail={detail.data} graph={graph.data} items={items} ranks={ranks} />
          <ItemTabs
            path={path}
            tab={tab}
            detail={detail.data}
            live={items}
            ranks={ranks}
            opened={opened.data.items}
            isCut={opened.data.next_cursor != null}
            now={now}
          />
        </>
      )}
    </main>
  );
}
