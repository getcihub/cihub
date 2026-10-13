import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

/** A link to the drawer of the PR `number`, in the queue of the page, with its tab. */
export function PrLink({
  number,
  className,
  children,
}: {
  number: number;
  className: string;
  children: ReactNode;
}) {
  return (
    <Link
      className={className}
      from="/orgs/$org/repos/$repo/queues/$queue"
      to="/orgs/$org/repos/$repo/queues/$queue/prs/$number"
      params={(kept) => ({ ...kept, number })}
      search={(kept) => kept}
    >
      {children}
    </Link>
  );
}
