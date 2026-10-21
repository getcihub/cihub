import { useState } from "react";

import type { Repo, SetupState } from "@/client/types.gen";
import { isSetupBlocked } from "@/domain/setup/queries";

/** A verdict of the setup that the server read; `null` before its first read. */
export type Verdict = Exclude<SetupState, "checking"> | null;

/**
 * The verdict of the setup of each repository of `repos`. While the server checks a setup again, the API gives no
 * verdict: the repository keeps the last one that the page saw, so that its row stays where it is.
 */
export function useSetupVerdicts(repos: Repo[]): (repo: Repo) => Verdict {
  const [seen, setSeen] = useState<Readonly<Record<string, Verdict>>>({});
  const verdictOf = (repo: Repo): Verdict =>
    repo.setup.state === "checking" ? (seen[repo.id] ?? null) : repo.setup.state;

  if (repos.some((repo) => verdictOf(repo) !== (seen[repo.id] ?? null))) {
    setSeen({ ...seen, ...Object.fromEntries(repos.map((repo) => [repo.id, verdictOf(repo)])) });
  }

  return verdictOf;
}

/** A point of the setup blocks the first queue, as the server last read it. */
export function isBlockedVerdict(verdict: Verdict): boolean {
  return verdict !== null && isSetupBlocked(verdict);
}
