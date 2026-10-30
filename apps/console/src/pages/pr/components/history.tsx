import type { Event } from "@/client/types.gen";
import { Prose } from "@cihub/ui/components/prose";
import { cn } from "@cihub/ui/lib/cn";
import { itemStateLabel } from "@/domain/pr/words";
import { useNow } from "@/hooks/use-now";
import { dateAndTime, timeOfDay } from "@/lib/time";
import { BlockReasons } from "./block-reasons";
import { DrawerSection, DrawerSectionTitle } from "./drawer-frame";

/** The dot of a transition, in the color of the state that it reaches, as the mockups v26; the others stay grey. */
const TRANSITION_DOTS: Record<string, string> = {
  testing: "before:border-primary",
  merged: "before:border-primary",
  tests_passed: "before:border-success",
  pending_failure: "before:border-warning",
  failed: "before:border-destructive",
};

/** The name of an event of the history: the submission, the state that a transition reaches, else what happened. */
function whatOf(event: Event): string {
  switch (event.type) {
    case "item.transitioned":
      // The first transition of an item has no state before it: it is the submission.
      return event.payload.from == null ? "Submitted" : itemStateLabel(String(event.payload.to));
    case "item.readiness_changed":
      return blockReasonsOf(event).length > 0 ? "Blocked" : "Ready";
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
  switch (event.type) {
    case "item.transitioned":
      return TRANSITION_DOTS[String(event.payload.to)];
    // A block in amber, as the reasons of a PR that waits in the mockups v26.
    case "item.readiness_changed":
      return blockReasonsOf(event).length > 0 ? "before:border-warning" : "before:border-success";
    case "merge.succeeded":
      return "before:border-primary";
    case "merge.failed":
      return "before:border-destructive";
    default:
      return undefined;
  }
}

/** The reasons that an `item.readiness_changed` gives; none for another event. */
function blockReasonsOf(event: Event): Record<string, unknown>[] {
  const reasons = event.payload.block_reasons;
  if (event.type !== "item.readiness_changed" || !Array.isArray(reasons)) {
    return [];
  }
  return reasons.filter(
    (reason): reason is Record<string, unknown> => typeof reason === "object" && reason !== null,
  );
}

/** What an event says: the sentence of the API, or what blocks the PR, each queue condition as the condition builder. */
function EventDetail({ event }: { event: Event }) {
  const reasons = blockReasonsOf(event);
  if (reasons.length === 0) {
    return (
      <div className="wrap-anywhere text-secondary-foreground">
        <Prose text={event.description} />
        {event.actor && `, by @${event.actor}`}
      </div>
    );
  }

  return (
    <div className="mt-1 wrap-anywhere text-secondary-foreground">
      <BlockReasons reasons={reasons} className="gap-1.5" />
      {event.actor && <div className="mt-1">by @{event.actor}</div>}
    </div>
  );
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
            <EventDetail event={event} />
          </li>
        ))}
      </ol>
    </DrawerSection>
  );
}
