import type { Pr } from "@/client/types.gen";
import { cn } from "@cihub/ui/lib/cn";
import { PrLink } from "./pr-link";

const DOTS: Record<string, string> = {
  not_ready: "border-2 border-dashed border-waiting",
  pending: "border-2 border-waiting",
  testing: "animate-spin border-2 border-primary-border border-t-primary motion-reduce:animate-none",
  pending_failure: "bg-warning",
  tests_passed: "bg-success",
};

export function Chip({ item, text, rank }: { item: Pr; text: string | null; rank: number | undefined }) {
  return (
    <li data-flip={item.id}>
      <PrLink
        className="flex items-center gap-2 rounded-sm border border-border bg-card px-2.25 py-1.25 text-md whitespace-nowrap text-inherit hover:border-muted-foreground focus-visible:border-muted-foreground"
        number={item.number}
      >
        {rank !== undefined && (
          <span className="grid h-4 min-w-4 place-items-center rounded-xs bg-muted px-1 text-2xs font-medium text-secondary-foreground tabular-nums">
            {rank}
          </span>
        )}
        <span className={cn("size-2 flex-none rounded-full", DOTS[item.state])} />
        <b className="font-semibold">#{item.number}</b>
        {text && <span className="max-w-37.5 truncate text-secondary-foreground">{text}</span>}
      </PrLink>
    </li>
  );
}
