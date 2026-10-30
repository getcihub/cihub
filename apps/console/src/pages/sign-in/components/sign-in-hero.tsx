import type { ReactNode } from "react";

import { CheckIcon } from "@cihub/icons/check";
import { Brand } from "@/components/session/parts";
import { Timeline } from "./timeline";

/** The left side of the sign-in: what CIHub promises, beside the form. Hidden in a narrow window. */
export function SignInHero() {
  return (
    <section className="flex flex-col justify-center overflow-hidden border-r border-border bg-muted bg-dots p-14 max-lg:hidden">
      <Brand />
      <h2 className="mt-10 mb-3 text-7xl font-semibold tracking-[-0.6px]">Never break main again.</h2>
      <p className="max-w-115 text-2xl text-secondary-foreground">
        Every pull request is tested on top of everything that lands before it, and merges only if the result
        still passes.
      </p>
      <Timeline className="mt-12 h-auto w-full max-w-140" />
      <ul className="mt-9 grid gap-3.5">
        <Benefit>
          What is tested is exactly what lands on <code>main</code>.
        </Benefit>
        <Benefit>Your CI, unchanged: CIHub waits for its required checks.</Benefit>
        <Benefit>Every pull request says why it waits.</Benefit>
      </ul>
    </section>
  );
}

function Benefit({ children }: { children: ReactNode }) {
  return (
    <li className="flex items-center gap-3 text-lg text-secondary-foreground">
      <CheckIcon className="size-5 flex-none text-primary" />
      <span>{children}</span>
    </li>
  );
}
