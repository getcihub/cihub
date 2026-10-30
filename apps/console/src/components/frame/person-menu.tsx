import { useState } from "react";

import { CheckIcon } from "@cihub/icons/check";
import { ExternalIcon } from "@cihub/icons/external";
import { Avatar } from "@cihub/ui/components/avatar";
import {
  Menu,
  MenuHint,
  MenuItem,
  menuItemVariants,
  MenuLabel,
  MenuSeparator,
  MenuSub,
} from "@cihub/ui/components/menu";
import { cn } from "@cihub/ui/lib/cn";
import { type Me } from "@/domain/session/queries";
import { type Theme, chooseTheme, chosenTheme } from "@/lib/preferences";

const REPOSITORY_URL = "https://github.com/getcihub/cihub";

/** A page outside the dashboard, in a new tab. */
function ExternalLink({ href, label, hint }: { href: string; label: string; hint?: string }) {
  return (
    <a className={menuItemVariants()} href={href} target="_blank" rel="noreferrer">
      {label}
      {hint && <MenuHint>{hint}</MenuHint>}
      <ExternalIcon className={cn("size-3.5 flex-none text-muted-foreground", !hint && "ml-auto")} />
    </a>
  );
}

const THEMES: { theme: Theme; label: string }[] = [
  { theme: "light", label: "Light" },
  { theme: "dark", label: "Dark" },
  { theme: "system", label: "System" },
];

export function PersonMenu({
  me: { login, name, avatar_url, version },
  onSignOut,
}: {
  me: Me;
  onSignOut: () => void;
}) {
  const [theme, setTheme] = useState(chosenTheme);

  return (
    <Menu
      label="Your account"
      trigger={
        <>
          <Avatar name={login} src={avatar_url} size={24} />
          <span className="sr-only">{login}</span>
        </>
      }
      triggerClassName="flex cursor-pointer items-center rounded-full p-0.5 text-foreground hover:bg-muted"
      align="right"
    >
      <MenuLabel>
        <div className="font-medium">{name ?? login}</div>
        {name && <div className="text-sm text-muted-foreground">@{login}</div>}
      </MenuLabel>
      <MenuSeparator />
      <MenuSub label="Appearance">
        {THEMES.map((choice) => (
          <MenuItem
            key={choice.theme}
            aria-pressed={choice.theme === theme}
            onClick={() => {
              chooseTheme(choice.theme);
              setTheme(choice.theme);
            }}
          >
            {choice.label}
            {choice.theme === theme && <CheckIcon className="ml-auto size-4 flex-none" />}
          </MenuItem>
        ))}
      </MenuSub>
      <MenuSeparator />
      <ExternalLink href={`${REPOSITORY_URL}#readme`} label="Documentation" />
      <ExternalLink href={`${REPOSITORY_URL}/releases/tag/v${version}`} label="About" hint={`v${version}`} />
      <ExternalLink href={REPOSITORY_URL} label="GitHub" />
      <MenuSeparator />
      <MenuItem onClick={onSignOut}>Sign out</MenuItem>
    </Menu>
  );
}
