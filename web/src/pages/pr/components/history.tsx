import type { Event } from "@/client/types.gen";
import { Prose } from "@/components/ui/prose";
import { itemStateLabel } from "@/domain/pr/words";
import { dateAndTime } from "@/lib/time";
import { DrawerSection, DrawerSectionTitle } from "./drawer-frame";

/** The name of an event of the history: the state that a transition reaches, else what happened. */
function whatOf(event: Event): string {
  switch (event.type) {
    case "item.transitioned":
      return itemStateLabel(String(event.payload.to));
    case "item.readiness_changed":
      return "Readiness";
    case "item.cancel_requested":
      return "Cancellation asked";
    case "item.restart_requested":
      return "New test asked";
    case "merge.succeeded":
      return "Merge";
    case "merge.failed":
      return "Merge failed";
    default:
      return "Event";
  }
}

/** What happened to the PR, the oldest first, with who did it. */
export function History({ events }: { events: Event[] }) {
  return (
    <DrawerSection aria-labelledby="history-title">
      <DrawerSectionTitle id="history-title">History</DrawerSectionTitle>
      <ol className="relative before:absolute before:top-2 before:bottom-2 before:left-1.75 before:w-0.5 before:bg-divider">
        {events.map((event) => (
          <li
            key={event.id}
            className="relative pb-3.5 pl-7 text-md before:absolute before:top-1 before:left-0.5 before:size-3 before:rounded-full before:border-2 before:border-waiting before:bg-card"
          >
            <div className="text-sm text-muted-foreground">{dateAndTime(event.occurred_at)}</div>
            <div className="font-semibold">{whatOf(event)}</div>
            <div className="wrap-anywhere text-secondary-foreground">
              <Prose text={event.description} />
              {event.actor && <span className="text-muted-foreground"> · by @{event.actor}</span>}
            </div>
          </li>
        ))}
      </ol>
    </DrawerSection>
  );
}
