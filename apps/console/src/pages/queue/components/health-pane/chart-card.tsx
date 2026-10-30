import type { ReactNode } from "react";

import { Card, CardHeader, CardHint, CardTitle } from "@cihub/ui/components/card";
import { cn } from "@cihub/ui/lib/cn";

export function Key({ swatch, label }: { swatch: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.25">
      <i className={cn("inline-block size-2 flex-none rounded-xs", swatch)} />
      {label}
    </span>
  );
}

export function ChartCard({
  title,
  keys,
  children,
}: {
  title: string;
  keys: ReactNode[];
  children: ReactNode;
}) {
  return (
    <Card className="min-w-0">
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        <CardHint className="flex gap-3">{keys}</CardHint>
      </CardHeader>
      {children}
    </Card>
  );
}
