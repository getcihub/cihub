import { Link } from "@tanstack/react-router";
import { type ReactNode } from "react";

import { Mark } from "@cihub/icons/brand/mark";

/** The bar at the top of a page, with the logo of CIHub first. */
export function TopBar({ children }: { children: ReactNode }) {
  return (
    <header className="sticky top-0 z-10 flex h-13 items-center gap-1 border-b border-border bg-card px-page max-md:gap-0.5 max-md:px-3.5">
      {children}
    </header>
  );
}

/** The logo of the top bar: a link to the queues of `org`, or the logo alone while a page loads. */
export function TopBarLogo({ org }: { org?: string }) {
  const place = "mr-2 grid place-items-center max-md:mr-0.5";
  const logo = <Mark className="size-6" />;

  if (!org) {
    return <span className={place}>{logo}</span>;
  }

  return (
    <Link className={place} to="/orgs/$org" params={{ org }} aria-label="CIHub">
      {logo}
    </Link>
  );
}
