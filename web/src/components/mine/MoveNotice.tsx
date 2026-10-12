import { useQueryClient } from "@tanstack/react-query";
import { AnimatePresence, m } from "motion/react";
import { useEffect, useState } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { whereAmong } from "@/domain/pr/words";
import { Button } from "@/ui/button";
import { transitions } from "@/ui/motion";
import { minePrsOf, minePrsReads } from "@/domain/mine/queries";

const SHOWN_MS = 8_000;

interface PrPlace {
  /** "#124 Fix the login redirect". */
  title: string;
  /** "testing", "2nd in line". */
  where: string;
}

/** Where each PR of `prs` is, by the id of the PR. */
function placesOf(prs: Pr[], queues: Queue[]): Map<string, PrPlace> {
  return new Map(
    prs.map((pr) => [
      pr.id,
      { title: `#${String(pr.number)} ${pr.title ?? ""}`, where: whereAmong(pr, queues) },
    ]),
  );
}

/**
 * Tells the person, for a moment, which of their PRs moved: "#124 Fix the login redirect: testing". It compares
 * the reads of their PRs in the cache, the last one with the new one.
 */
export function MoveNotice({ org, queues }: { org: string; queues: Queue[] }) {
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    const reads = minePrsReads(org);
    const keys = [JSON.stringify(reads.live.queryKey), JSON.stringify(reads.merged.queryKey)];
    const mine = () =>
      minePrsOf(
        queryClient.getQueryData(reads.live.queryKey)?.prs,
        queryClient.getQueryData(reads.merged.queryKey)?.prs,
      );
    let lastPlaces = placesOf(mine(), queues);

    return queryClient.getQueryCache().subscribe((event) => {
      if (event.type !== "updated" || !keys.includes(JSON.stringify(event.query.queryKey))) {
        return;
      }
      const places = placesOf(mine(), queues);
      const moved = [...places].flatMap(([id, place]) => {
        const before = lastPlaces.get(id);
        return before !== undefined && before.where !== place.where ? [place] : [];
      });
      lastPlaces = places;
      if (moved.length > 0) {
        setNotice(moved.map((place) => `${place.title}: ${place.where}`).join(" · "));
      }
    });
  }, [org, queues, queryClient]);

  useEffect(() => {
    if (!notice) {
      return;
    }
    const timer = setTimeout(() => {
      setNotice(null);
    }, SHOWN_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [notice]);

  return (
    <div className="fixed right-5 bottom-5 z-70" role="status">
      <AnimatePresence>
        {notice && (
          <m.p
            key="notice"
            className="rounded-md border border-primary-border bg-card px-3 py-2.25 text-base shadow-lg"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 12 }}
            transition={transitions.slide}
          >
            {notice}{" "}
            <Button
              onClick={() => {
                setNotice(null);
              }}
            >
              Dismiss
            </Button>
          </m.p>
        )}
      </AnimatePresence>
    </div>
  );
}
