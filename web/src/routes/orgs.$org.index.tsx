import { useSuspenseQuery } from "@tanstack/react-query";
import { Link, createFileRoute } from "@tanstack/react-router";

import { listOrgQueuesOptions, meOptions } from "../client/@tanstack/react-query.gen";
import { EmptyState } from "../ui/EmptyState";
import { ArrowIcon } from "../ui/icons";
import styles from "../ui/ui.module.css";

export const Route = createFileRoute("/orgs/$org/")({
  component: OrgPage,
});

// The cards of the queues replace the list in lot 10b.
function OrgPage() {
  const { org } = Route.useParams();
  const { data: me } = useSuspenseQuery(meOptions());
  const { data } = useSuspenseQuery(listOrgQueuesOptions({ path: { org } }));

  return (
    <main className={styles.page}>
      <div className={styles.head}>
        <div>
          <div className={styles.sub}>{org}</div>
          <h1>Queues</h1>
        </div>
      </div>
      <div className={styles.card}>
        {data.queues.length > 0 ? (
          <ul>
            {data.queues.map(({ repo, queue }) => (
              <li key={queue.id}>
                <Link to="/orgs/$org/repos/$repo/queues/$queue" params={{ org, repo, queue: queue.name }}>
                  {repo} · {queue.name}
                </Link>
              </li>
            ))}
          </ul>
        ) : (
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
        )}
      </div>
    </main>
  );
}
