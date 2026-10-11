import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { listOrgQueuesOptions, meOptions } from "../client/@tanstack/react-query.gen";
import { READ_AGAIN_EVERY } from "../live";
import { QueueCards } from "../overview/QueueCards";
import { EmptyState } from "../ui/EmptyState";
import { ArrowIcon } from "../ui/icons";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import styles from "../ui/ui.module.css";

export const Route = createFileRoute("/orgs/$org/")({
  component: OrgPage,
  // A failed read shows under the frame, which stays.
  errorComponent: ServerErrorPage,
});

function OrgPage() {
  const { org } = Route.useParams();
  const { data: me } = useSuspenseQuery(meOptions());
  const { data } = useSuspenseQuery({
    ...listOrgQueuesOptions({ path: { org } }),
    refetchInterval: READ_AGAIN_EVERY,
  });

  if (data.queues.length > 0) {
    return (
      <main className={styles.page}>
        <div className={styles.head}>
          <div>
            <div className={styles.sub}>{org}</div>
            <h1>Queues</h1>
          </div>
        </div>
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
