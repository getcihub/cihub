import { hashKey, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import type { Pr, Queue } from "@/client/types.gen";
import { Button } from "@cihub/ui/components/button";
import { Toast } from "@cihub/ui/components/toast";
import { mineLiveOptions, mineMergedOptions, minePrsOf } from "@/domain/mine/queries";
import { whereAmong } from "@/domain/pr/words";
import { useMe } from "@/hooks/use-me";
import { useTimedValue } from "@/hooks/use-timed-value";

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
  const { login } = useMe();

  const [notice, setNotice] = useTimedValue<string>(SHOWN_MS);

  useEffect(() => {
    const live = mineLiveOptions(org, login);
    const merged = mineMergedOptions(org, login);
    const hashes = [hashKey(live.queryKey), hashKey(merged.queryKey)];
    const mine = () =>
      minePrsOf(queryClient.getQueryData(live.queryKey)?.prs, queryClient.getQueryData(merged.queryKey)?.prs);
    let lastPlaces = placesOf(mine(), queues);

    return queryClient.getQueryCache().subscribe((event) => {
      if (event.type !== "updated" || !hashes.includes(event.query.queryHash)) {
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
  }, [org, login, queues, queryClient, setNotice]);

  return (
    <Toast>
      {notice && (
        <>
          {notice}{" "}
          <Button
            onClick={() => {
              setNotice(null);
            }}
          >
            Dismiss
          </Button>
        </>
      )}
    </Toast>
  );
}
