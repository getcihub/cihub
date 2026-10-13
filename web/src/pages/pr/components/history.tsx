import type { Event } from "@/client/types.gen";
import { Prose } from "@/components/ui/prose";
import { itemStateLabel } from "@/domain/pr/words";
import { useNow } from "@/hooks/use-now";
import { cn } from "@/lib/cn";
import { dateAndTime, timeOfDay } from "@/lib/time";
import { DrawerSection, DrawerSectionTitle } from "./drawer-frame";

/** The dot of a transition, in the color of the state that it reaches; the others stay grey. */
const TRANSITION_DOTS: Record<string, string> = {
  testing: "before:border-primary",
  merged: "before:border-primary",
  tests_passed: "before:border-success",
  pending_failure: "before:border-warning",
  failed: "before:border-destructive",
};

/** The name of an event of the history: the state that a transition reaches, else what happened. */
function whatOf(event: Event): string {
  switch (event.type) {
    case "item.transitioned":
      return itemStateLabel(String(event.payload.to));
    case "item.readiness_changed":
      return Array.isArray(event.payload.block_reasons) && event.payload.block_reasons.length > 0
        ? "Waiting"
        : "Ready";
    case "item.cancel_requested":
      return "Cancel asked";
    case "item.restart_requested":
      return "Restart asked";
    case "merge.succeeded":
      return "Merge succeeded";
    case "merge.failed":
      return "Merge failed";
    default:
      return "Event";
  }
}

function dotOf(event: Event): string | undefined {
  if (event.type === "merge.failed") {
    return "before:border-destructive";
  }
  return event.type === "item.transitioned" ? TRANSITION_DOTS[String(event.payload.to)] : undefined;
}

/** Whether `at`, an RFC 3339 time, is on the day of `now`, in the time zone of the person. */
function isToday(at: string, now: number): boolean {
  return new Date(at).toDateString() === new Date(now).toDateString();
}

/** What happened to the PR, the newest first, with who did it. */
export function History({ events }: { events: Event[] }) {
  const now = useNow();
  // The API gives the oldest first.
  const newestFirst = [...events].reverse();

  return (
    <DrawerSection aria-labelledby="history-title">
      <DrawerSectionTitle id="history-title">History</DrawerSectionTitle>
      <ol className="relative before:absolute before:top-2 before:bottom-2 before:left-1.75 before:w-0.5 before:bg-divider">
        {newestFirst.map((event) => (
          <li
            key={event.id}
            className={cn(
              "relative pb-3.5 pl-7 text-md before:absolute before:top-1 before:left-0.5 before:size-3 before:rounded-full before:border-2 before:border-waiting before:bg-card",
              dotOf(event),
            )}
          >
            <div className="text-sm text-muted-foreground">
              {isToday(event.occurred_at, now)
                ? timeOfDay(event.occurred_at)
                : dateAndTime(event.occurred_at)}
            </div>
            <div className="font-semibold">{whatOf(event)}</div>
            <div className="wrap-anywhere text-secondary-foreground">
              <Prose text={event.description} />
              {event.actor && `, by @${event.actor}`}
            </div>
          </li>
        ))}
      </ol>
    </DrawerSection>
  );
}
