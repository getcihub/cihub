import { type ReactNode } from "react";

import type { Queue } from "@/client/types.gen";
import { type Me } from "@/domain/session/queries";
import { useSignOut } from "@/hooks/use-sign-out";
import { TopBar, TopBarLogo } from "./top-bar";
import { OrgPicker, QueuePicker, RepoPicker } from "./pickers";
import { PersonMenu } from "./person-menu";

interface Props {
  me: Me;
  /** The organization of the URL. */
  org: string;
  /** Its queues, by repository then target branch; empty once access is lost. */
  queues: Queue[];
  /** The names of its repositories, those without a queue too; none while they load. */
  repos?: string[];
  /** The repository and the queue of the URL, if any. */
  repo?: string | undefined;
  queue?: string | undefined;
  children: ReactNode;
}

/** The top bar of every page of an organization: CIHub / organization / repository / queue. */
export function Frame({ me, org, queues, repos = [], repo, queue, children }: Props) {
  const { startSignOut, hasSignOutFailed } = useSignOut();

  const names = [...new Set([...repos, ...queues.map((candidate) => candidate.repo)])].sort();
  // The frame names only a repository and a queue that exist: a wrong address shows "not found" under it.
  const knownRepo = repo !== undefined && names.includes(repo) ? repo : undefined;
  const queuesOfRepo = queues.filter((candidate) => candidate.repo === knownRepo);
  const knownQueue = queuesOfRepo.some((candidate) => candidate.target_branch === queue) ? queue : undefined;

  return (
    <>
      <TopBar>
        <TopBarLogo org={org} />
        <nav className="flex min-w-0 items-center gap-0.5 font-medium" aria-label="Where you are">
          <OrgPicker me={me} org={org} />
          {names.length > 0 && (
            <>
              <span className="text-waiting">/</span>
              <RepoPicker org={org} names={names} queues={queues} repo={knownRepo} />
            </>
          )}
          {knownRepo && knownQueue && (
            <>
              <span className="text-waiting">/</span>
              <QueuePicker org={org} repo={knownRepo} queues={queuesOfRepo} queue={knownQueue} />
            </>
          )}
        </nav>
        <div className="ml-auto">
          <PersonMenu me={me} onSignOut={startSignOut} />
        </div>
      </TopBar>
      {hasSignOutFailed && (
        <p
          className="border-b border-destructive-border bg-destructive-muted px-page py-2.5 text-base font-medium text-destructive"
          role="alert"
        >
          The sign-out failed. Reload the page, then try again.
        </p>
      )}
      {children}
    </>
  );
}
