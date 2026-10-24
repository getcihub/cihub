import type { Strategy } from "@/client/types.gen";
import { cn } from "@/lib/cn";

// The drawings of the strategies of the mockups v26: the branch, then the runs in flight on it.
const MAIN = "fill-primary";
const LINE = "fill-none stroke-waiting stroke-[1.5]";
const TEXT = "fill-secondary-foreground font-mono";

function Run({
  x,
  y,
  width,
  height,
  label,
  isChosen,
}: {
  x: number;
  y: number;
  width: number;
  height: number;
  label?: string;
  isChosen: boolean;
}) {
  return (
    <>
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        rx={3}
        className={cn("fill-card stroke-[1.5]", isChosen ? "stroke-primary" : "stroke-primary-border")}
      />
      {label && (
        <text x={x + width / 2} y={y + height / 2 + 3.5} fontSize={9} textAnchor="middle" className={TEXT}>
          {label}
        </text>
      )}
    </>
  );
}

/** What a strategy does, drawn: one run, runs in a line, batches, or lines of batches. */
export function StrategyArt({ strategy, isChosen }: { strategy: Strategy; isChosen: boolean }) {
  const run = (x: number, y: number, width: number, height: number, label?: string) => (
    <Run x={x} y={y} width={width} height={height} label={label} isChosen={isChosen} />
  );

  return (
    <div className="mt-1.5 rounded-sm bg-muted p-2">
      <svg viewBox="0 0 120 40" className="block h-10 w-full" aria-hidden="true">
        <circle cx={12} cy={20} r={5} className={MAIN} />
        {strategy === "sequential" && (
          <>
            <path d="M17 20 H34" className={LINE} />
            {run(34, 12, 22, 16, "1")}
          </>
        )}
        {strategy === "pipelined" && (
          <>
            <path d="M17 20 H112" className={LINE} />
            {run(26, 12, 22, 16, "1")}
            {run(56, 12, 22, 16, "2")}
            {run(86, 12, 22, 16, "3")}
          </>
        )}
        {strategy === "batched" && (
          <>
            <path d="M17 20 H112" className={LINE} />
            {run(26, 9, 36, 22, "1 2 3")}
            {run(70, 9, 36, 22, "4 5")}
          </>
        )}
        {strategy === "parallel" && (
          <>
            <path d="M17 20 C26 20 26 9 36 9 H110 M17 20 C26 20 26 31 36 31 H110" className={LINE} />
            {run(40, 2, 30, 14, "1 2")}
            {run(40, 24, 30, 14, "3")}
          </>
        )}
      </svg>
    </div>
  );
}
