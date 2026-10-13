import type { ReactNode } from "react";

interface Props {
  illustration: ReactNode;
  title: string;
  /** `h1` when the empty state is the whole page. */
  heading?: "h1" | "h3";
  children: ReactNode;
  actions?: ReactNode;
}

/** A page or a card with nothing to show: what happened, and what to do. */
export function EmptyState({ illustration, title, heading: Heading = "h3", children, actions }: Props) {
  return (
    <div data-slot="empty-state" className="grid justify-items-center gap-2 px-6 py-11 text-center">
      {illustration}
      <Heading className="text-2xl font-medium">{title}</Heading>
      <p className="max-w-115 text-lg text-secondary-foreground">{children}</p>
      {actions && <div className="mt-3 flex items-center gap-2 max-md:flex-col">{actions}</div>}
    </div>
  );
}
