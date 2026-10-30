import { ThemeSwitch } from "fumadocs-ui/layouts/shared/slots/theme-switch";
import Link from "next/link";

import { Mark } from "@cihub/icons/brand/mark";
import { changelogUrl, repositoryUrl } from "@/lib/shared";

import { GitHubIcon } from "./icons";

export function SiteHeader() {
  return (
    <header className="nav">
      <div className="wrap in">
        <Link className="brand" href="/">
          <Mark className="mark" />
          CIHub
        </Link>
        <nav className="links">
          <a href="#how">How it works</a>
          <a href="#features">Features</a>
          <a href="#config">Configuration</a>
          <Link href="/docs">Docs</Link>
          <a href={changelogUrl}>Changelog</a>
        </nav>
        <div className="right">
          <a className="gh" href={repositoryUrl}>
            <GitHubIcon />
            GitHub
          </a>
          <ThemeSwitch />
          <Link className="btn small primary" href="/docs/install">
            Install CIHub
          </Link>
        </div>
      </div>
    </header>
  );
}
