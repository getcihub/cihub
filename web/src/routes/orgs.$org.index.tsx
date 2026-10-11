import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { MinePrs } from "../mine/Mine";
import { useMinePrs } from "../mine/reads";
import { QueueCards } from "../overview/QueueCards";
import { EmptyState } from "../ui/EmptyState";
import { ArrowIcon } from "../ui/icons";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import styles from "../ui/ui.module.css";
import { useMe } from "../session";
import { orgQueuesOptions } from "../queue/reads";

export const Route = createFileRoute("/orgs/$org/")({
  component: OrgPage,
  // A failed read shows under the frame, which stays.
  errorComponent: ServerErrorPage,
});

function OrgPage() {
  const { org } = Route.useParams();
  const me = useMe();
  const { data } = useSuspenseQuery(orgQueuesOptions(org));
  const mine = useMinePrs(org);

  if (data.queues.length > 0) {
    return (
      <main className={styles.page}>
        <div className={styles.head}>
          <div>
            <div className={styles.sub}>{org}</div>
            <h1>Queues</h1>
          </div>
        </div>
        <MinePrs org={org} prs={mine} queues={data.queues} />
        <QueueCards org={org} queues={data.queues} />
      </main>
    );
  }

  return (
    <main className={styles.page}>
      <div className={styles.head}>
        <div>
          <div className={styles.sub}>{org}</div>
          <h1>Queues</h1>
        </div>
      </div>
      <div className={styles.card}>
        <EmptyState
          illustration={null}
          title="No queue yet"
          actions={
            me.app_install_url && (
              <a className={styles.button} href={me.app_install_url}>
                Choose the repositories of the App <ArrowIcon />
              </a>
            )
          }
        >
          The CIHub App is installed on <b>{org}</b>. The queue of a branch appears here at the first{" "}
          <code>/cihub merge</code> on one of its pull requests.
        </EmptyState>
      </div>
    </main>
  );
}
