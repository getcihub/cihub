import Link from "next/link";

import { Mark } from "@/components/mark";
import { changelogUrl, repositoryUrl, version } from "@/lib/shared";

export function SiteFooter() {
  return (
    <footer className="foot">
      <div className="wrap">
        <div className="in">
          <div className="about">
            <Link className="brand" href="/">
              <Mark className="mark" />
              CIHub
            </Link>
            <span>The self-hosted merge queue for GitHub. Never break main again.</span>
          </div>
          <div>
            <h4>Product</h4>
            <ul>
              <li>
                <a href="#how">How it works</a>
              </li>
              <li>
                <a href="#features">Features</a>
              </li>
              <li>
                <a href="#config">Configuration</a>
              </li>
              <li>
                <a href="#compare">Compare</a>
              </li>
            </ul>
          </div>
          <div>
            <h4>Docs</h4>
            <ul>
              <li>
                <Link href="/docs">What is CIHub</Link>
              </li>
              <li>
                <Link href="/docs/install">Install</Link>
              </li>
              <li>
                <Link href="/docs/agents">Coding agents</Link>
              </li>
              <li>
                <a href="/llms.txt">llms.txt</a>
              </li>
            </ul>
          </div>
          <div>
            <h4>Project</h4>
            <ul>
              <li>
                <a href={repositoryUrl}>GitHub</a>
              </li>
              <li>
                <a href={changelogUrl}>Changelog</a>
              </li>
              <li>
                <a href={`${repositoryUrl}/blob/main/GOAL.md`}>Why CIHub</a>
              </li>
              <li>
                <a href={`${repositoryUrl}/blob/main/AGENTS.md`}>Contribute</a>
              </li>
            </ul>
          </div>
        </div>
        <div className="legal">
          <span>FSL-1.1-ALv2. Each version becomes Apache-2.0 two years after its release.</span>
          <span>v{version} · Early days</span>
        </div>
      </div>
    </footer>
  );
}
