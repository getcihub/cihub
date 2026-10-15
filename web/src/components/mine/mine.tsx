import { Link } from "@tanstack/react-router";
import { useRef } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { CardHeader, CardHint, CardTitle, cardVariants } from "@/components/ui/card";
import { useFlip } from "@/components/ui/hooks/use-flip";
import { Prose } from "@/components/ui/prose";
import { etaOf, whereAmong, whereOf } from "@/domain/pr/words";
import { cn } from "@/lib/cn";
import { plural } from "@/lib/text";
import { Train } from "./train";

function PrTitle({ org, pr, isNumberBold = false }: { org: string; pr: Pr; isNumberBold?: boolean }) {
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

/** The PRs of the person in the queue of the page, first on it. */
export function Spotlight({ org, prs, queue }: { org: string; prs: Pr[]; queue: Queue }) {
  const branch = queue.target_branch;

  return (
    <>
      {prs.map((pr) => {
        const eta = etaOf(pr);
        const before = pr.ahead.length;

        return (
          <section
            key={pr.id}
            className={cn(cardVariants({ tone: "primary" }), "grid gap-3 px-4 py-3.5")}
            aria-label="Your pull request"
          >
            <div className="flex flex-wrap items-baseline gap-2.5">
              <span className="text-sm font-medium text-primary">Your pull request</span>
              <PrTitle org={org} pr={pr} isNumberBold />
              {eta && (
                <span className="ml-auto text-md text-muted-foreground">
                  Merges in <b className="font-medium text-foreground">{eta}</b>
                </span>
              )}
            </div>
            <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,320px)] items-center gap-6 max-md:grid-cols-[minmax(0,1fr)] max-md:gap-2">
              <div>
                <b className="block text-6xl leading-[1.15] font-medium tracking-[-0.4px]">
                  {whereOf(pr, queue.state === "paused", branch)}
                </b>
                {pr.state === "pending" && (
                  <span className="text-md text-muted-foreground">
                    {before === 0
                      ? "next to be tested"
                      : `${plural(before, "pull request", "pull requests")} merge before it`}
                  </span>
                )}
              </div>
              <Train pr={pr} branch={branch} />
            </div>
            {pr.state !== "merged" && (
              <p className="text-md text-secondary-foreground">
                <Prose text={pr.state_description} />
              </p>
            )}
          </section>
        );
      })}
    </>
  );
}
