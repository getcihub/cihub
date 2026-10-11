import { Link } from "@tanstack/react-router";

import type { Item, QueueDetail } from "../client/types.gen";
import { durationBetween, timeAgo } from "../time";
import { Avatar } from "../ui/Avatar";
import { ItemStateBadge } from "../ui/StateBadge";
import ui from "../ui/ui.module.css";
import { PrLink } from "./PrLink";
import { Prose } from "./Prose";
import styles from "./queue.module.css";
import type { QueuePath, Tab } from "./reads";
import { TABS } from "./reads";

const DAY = 24 * 60 * 60 * 1000;

const TAB_LABELS: Record<Tab, string> = {
  live: "In the queue",
  merged: "Merged",
  ended: "Failed and cancelled",
};

const NONE: Record<Tab, string> = {
  live: "No pull request in the queue.",
  merged: "Nothing merged in the last 24 hours.",
  ended: "Nothing failed or cancelled in the last 24 hours.",
};

const PRIORITIES: Record<number, string> = { 0: "urgent", 10: "high", 100: "medium", 200: "low" };

function priorityOf(item: Item) {
  const name = PRIORITIES[item.priority];
  const tone = name === "urgent" ? styles.urgent : name === "high" ? styles.high : "";

  return <span className={`${styles.priority} ${tone}`}>{name ?? String(item.priority)}</span>;
}

/** The sentence of the API, and for a merge where and after how long it landed. */
function Detail({ item }: { item: Item }) {
  const landed = item.state === "merged" && item.merged_sha;

  return (
    <>
      <Prose text={item.state_description} />
      {landed && (
        <span className={styles.facts}>
          {" "}
          · <code>{item.merged_sha?.slice(0, 7)}</code> · after{" "}
          {durationBetween(item.submitted_at, item.state_changed_at)}
        </span>
      )}
    </>
  );
}

/** The items of the open tab: the live ones in their order, the others of the last 24 hours. */
export function ItemTabs({
  path,
  tab,
  detail,
  live,
  ranks,
  opened,
  isCut,
  now,
}: {
  path: QueuePath;
  tab: Tab;
  detail: QueueDetail;
  live: Item[];
  ranks: Map<string, number>;
  opened: Item[];
  /** The read of the tab stopped at 100 items: the oldest submissions are not in it. */
  isCut: boolean;
  now: number;
}) {
  const rows = tab === "live" ? live : opened.filter((item) => now - Date.parse(item.state_changed_at) < DAY);
  const counts: Partial<Record<Tab, number>> = { live: live.length, merged: detail.merged_last_24h };

  return (
    <section className={styles.card} aria-labelledby="items-title">
      <div className={styles.sectionHead}>
        <h2 id="items-title" className={ui.hidden}>
          Pull requests
        </h2>
        <nav className={styles.tabs} aria-label="Tabs of the pull requests">
          {TABS.map((candidate) => (
            <Link
              key={candidate}
              className={`${styles.tab} ${candidate === tab ? styles.activeTab : ""}`}
              to="/orgs/$org/repos/$repo/queues/$queue"
              params={path}
              search={{ tab: candidate }}
              aria-current={candidate === tab ? "page" : undefined}
            >
              {TAB_LABELS[candidate]}
              {counts[candidate] !== undefined && <span className={styles.count}>{counts[candidate]}</span>}
            </Link>
          ))}
        </nav>
        <div className={styles.right}>
          {tab === "live" && "By position: priority first, then arrival"}
          {tab !== "live" && (isCut ? "Last 24 hours, the 100 latest submissions only" : "Last 24 hours")}
        </div>
      </div>
      {rows.length === 0 ? (
        <p className={styles.inline}>{NONE[tab]}</p>
      ) : (
        <table className={styles.table}>
          <thead>
            <tr>
              <th>#</th>
              <th>Pull request</th>
              <th>State</th>
              <th>Details</th>
              <th>Priority</th>
              <th>{tab === "live" ? "Since" : "When"}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((item) => (
              <tr key={item.id}>
                <td className={styles.pos}>{tab === "live" ? (ranks.get(item.id) ?? "–") : ""}</td>
                <td>
                  <div className={styles.pr}>
                    <Avatar name={item.author} />
                    <div>
                      <PrLink className={styles.prTitle} number={item.pr_number}>
                        {item.title ?? `#${String(item.pr_number)}`}
                      </PrLink>
                      <div className={styles.meta}>
                        #{item.pr_number} · {item.author}
                      </div>
                    </div>
                  </div>
                </td>
                <td>
                  <ItemStateBadge state={item.state} />
                </td>
                <td className={styles.detail}>
                  <Detail item={item} />
                </td>
                <td>{priorityOf(item)}</td>
                <td className={styles.time}>
                  {timeAgo(tab === "live" ? item.submitted_at : item.state_changed_at, now)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
