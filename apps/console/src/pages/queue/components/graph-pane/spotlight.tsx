import type { Pr, Queue } from "@/client/types.gen";
import { PrTitle } from "@/components/mine/mine";
import { Train } from "@/components/mine/train";
import { cardVariants } from "@cihub/ui/components/card";
import { Prose } from "@cihub/ui/components/prose";
import { cn } from "@cihub/ui/lib/cn";
import { etaOf, whereOf } from "@/domain/pr/words";
import { plural } from "@/lib/text";

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
