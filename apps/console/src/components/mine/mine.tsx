import { Link } from "@tanstack/react-router";
import { useRef } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { CardHeader, CardHint, CardTitle, cardVariants } from "@cihub/ui/components/card";
import { useFlip } from "@cihub/ui/hooks/use-flip";
import { etaOf, whereAmong } from "@/domain/pr/words";
import { Train } from "./train";

/** The number and the title of `pr`, a link to its drawer. */
export function PrTitle({ org, pr, isNumberBold = false }: { org: string; pr: Pr; isNumberBold?: boolean }) {
  return (
    <Link
      className="font-medium text-foreground hover:text-primary focus-visible:text-primary"
      to="/orgs/$org/repos/$repo/queues/$queue/prs/$number"
      params={{ org, repo: pr.repo, queue: pr.queue, number: pr.number }}
    >
      {isNumberBold ? <b className="font-bold">#{pr.number}</b> : `#${String(pr.number)}`} {pr.title ?? ""}
    </Link>
  );
}

/** "Your pull requests" on the queues of an organization: where each one is, and when it should merge. */
export function MinePrs({ org, prs, queues }: { org: string; prs: Pr[]; queues: Queue[] }) {
  const list = useRef<HTMLUListElement>(null);
  useFlip(list);

  if (prs.length === 0) {
    return null;
  }

  return (
    <section className={cardVariants()} aria-labelledby="mine-title">
      <CardHeader className="border-b-0">
        <CardTitle id="mine-title">Your pull requests</CardTitle>
        <CardHint>Where each one is</CardHint>
      </CardHeader>
      <ul ref={list}>
        {prs.map((pr) => {
          const branch = pr.queue;
          const eta = etaOf(pr);

          return (
            <li
              key={pr.id}
              className="grid grid-cols-[minmax(0,1fr)_minmax(0,320px)_170px] items-center gap-5 border-t border-divider px-4 py-2.5 max-md:grid-cols-[minmax(0,1fr)] max-md:gap-2"
              data-flip={pr.id}
            >
              <div>
                <PrTitle org={org} pr={pr} />
                <div className="text-sm text-muted-foreground">
                  {pr.repo} · {pr.queue}
                </div>
              </div>
              <Train pr={pr} branch={branch} />
              <div className="grid justify-items-end text-md max-md:justify-items-start">
                <b className="font-medium">{whereAmong(pr, queues)}</b>
                {eta && <span className="text-muted-foreground">merges in {eta}</span>}
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
