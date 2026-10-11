import { useSuspenseQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";

import { listOrgQueuesOptions, meOptions } from "../client/@tanstack/react-query.gen";
import { MinePrs } from "../mine/Mine";
import { useMinePrs } from "../mine/reads";
import { QueueCards } from "../overview/QueueCards";
import { NotFoundPage } from "../pages/NotFoundPage";
import { ServerErrorPage } from "../pages/ServerErrorPage";
import styles from "../ui/ui.module.css";

export const Route = createFileRoute("/orgs/$org/repos/$repo/")({
  component: RepoPage,
  // A failed read shows under the frame, which stays.
  errorComponent: ServerErrorPage,
});

function RepoPage() {
  const { org, repo } = Route.useParams();
  const { data } = useSuspenseQuery(listOrgQueuesOptions({ path: { org } }));
  const { data: me } = useSuspenseQuery(meOptions());
  const mine = useMinePrs(org, me.login);
  const queues = data.queues.filter((candidate) => candidate.repo === repo);

  if (queues.length === 0) {
    return <NotFoundPage />;
  }

  return (
    <main className={styles.page}>
      <div className={styles.head}>
        <div>
          <div className={styles.sub}>
            {org}/{repo}
          </div>
          <h1>Queues</h1>
        </div>
      </div>
      <MinePrs org={org} prs={mine.filter((pr) => pr.repo === repo)} queues={queues} />
      <QueueCards org={org} queues={queues} />
    </main>
  );
}
