import type { Metadata } from "next";
import Link from "next/link";

import { JsonLd } from "@/components/json-ld";
import { Mark } from "@cihub/ui";
import { repositoryUrl, siteDescription, siteName, siteUrl, version } from "@/lib/shared";

import { CopyButton } from "./copy-button";
import { WithQueueDiagram, WithoutQueueDiagram } from "./diagrams";
import { CheckCircleIcon, CheckIcon, GitHubIcon, MergeIcon } from "./icons";
import { QueuePreview } from "./queue-preview";

export const metadata: Metadata = {
  alternates: {
    canonical: "/",
    types: { "text/markdown": "/llms.txt" },
  },
  openGraph: { url: "/" },
};

const helmInstall = "helm install cihub oci://ghcr.io/getcihub/charts/cihub";

// The .cihub.yaml of this repository, shortened.
const exampleConfig = `# yaml-language-server: $schema=https://cihub.io/schema/config/v1.json
version: 1
queues:
  - branch: main
    strategy: batched
    batching:
      size: 3
      max_wait_minutes: 1
    queue_conditions:
      draft: false
      labels:
        none: [do-not-merge]
      checks:
        success: [Build Success]
priority_rules:
  - name: renovate
    conditions:
      author: renovate[bot]
    priority: low`;

// What an agent reads, then what the CLI shows: `kind` is the class of the line in the terminal.
const agentSession: { kind?: "command" | "heading" | "output" | "live"; text: string }[] = [
  { kind: "command", text: "curl https://cihub.io/llms.txt" },
  { kind: "heading", text: "# CIHub" },
  { kind: "output", text: "> CIHub is a self-hosted merge queue for GitHub. It tests each" },
  { kind: "output", text: "> pull request on top of the ones ahead of it and merges it only" },
  { kind: "output", text: "> if the result passes." },
  { text: "" },
  { kind: "heading", text: "## Docs" },
  { text: "- [What is CIHub](https://cihub.io/docs.md)" },
  { text: "- [Install CIHub](https://cihub.io/docs/install.md)" },
  { text: "- [Use CIHub with a coding agent](https://cihub.io/docs/agents.md)" },
  { text: "" },
  { kind: "command", text: "cihub queue status" },
  { kind: "live", text: " main  Running · 5 in the queue · 3 testing" },
  { kind: "output", text: "  #121 Fix the login redirect   3rd in line" },
];

export default function HomePage() {
  return (
    <main>
      <JsonLd
        data={{
          "@context": "https://schema.org",
          "@type": "SoftwareApplication",
          name: siteName,
          description: siteDescription,
          url: siteUrl,
          applicationCategory: "DeveloperApplication",
          operatingSystem: "Linux",
          license: `${repositoryUrl}/blob/main/LICENSE`,
          sameAs: repositoryUrl,
          offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
        }}
      />

      <section className="hero">
        <div className="wrap">
          <div className="text">
            <Link className="pill" href="/docs#status">
              <span className="tag">v{version}</span>Early days: do not use it in production yet →
            </Link>
            <h1>
              Never break <em>main</em> again.
            </h1>
            <p className="lead">
              CIHub is the self-hosted merge queue for GitHub. It tests each pull request on top of everything
              that lands before it, and merges it only if the result still passes. With the CI you already
              have.
            </p>
            <div className="ctas">
              <Link className="btn primary" href="/docs/install">
                Install CIHub
              </Link>
              <Link className="btn" href="/docs">
                Read the docs
              </Link>
              <span className="cmd">
                <span className="p">$</span>
                <span>
                  <b>helm install</b> cihub oci://ghcr.io/getcihub/charts/cihub
                </span>
                <CopyButton text={helmInstall} />
              </span>
            </div>
            <div className="facts">
              <span>
                <CheckIcon />
                Open source
              </span>
              <span>
                <CheckIcon />
                One container and a Postgres
              </span>
              <span>
                <CheckIcon />
                Your code stays on your infrastructure
              </span>
            </div>
          </div>
          <QueuePreview />
        </div>
      </section>

      <div className="wrap strip">
        <span>CIHub never runs your CI. It waits for the checks you already have:</span>
        <div className="names">
          <span>
            <GitHubIcon />
            GitHub Actions
          </span>
          <span>Buildkite</span>
          <span>CircleCI</span>
          <span>Jenkins</span>
          <span className="any">and any CI that reports a check</span>
        </div>
      </div>

      <section className="block" id="problem">
        <div className="wrap">
          <div className="s-head">
            <span className="eyebrow">{"// the problem"}</span>
            <h2>
              Two green pull requests. One red <code>main</code>.
            </h2>
            <p>
              Two pull requests pass CI on their own. You merge both. <code>main</code> is red, and nobody
              knows why: each one was tested without the other.
            </p>
          </div>
          <div className="versus">
            <div className="vs">
              <div className="top">
                <h3>Without a merge queue</h3>
                <span className="badge red">
                  <span className="dot" />
                  main is red
                </span>
              </div>
              <WithoutQueueDiagram />
              <p>
                Each pull request was green on an old <code>main</code>. Nobody tested the two together before
                they landed.
              </p>
            </div>
            <div className="vs good">
              <div className="top">
                <h3>With CIHub</h3>
                <span className="badge blue">
                  <span className="dot" />
                  main stays green
                </span>
              </div>
              <WithQueueDiagram />
              <p>
                CIHub tests the exact commit that will land: <code>main</code>, the pull requests ahead, and
                this one. If it fails, nothing lands.
              </p>
            </div>
          </div>
        </div>
      </section>

      <section className="block" id="how">
        <div className="wrap">
          <div className="s-head">
            <span className="eyebrow">{"// how it works"}</span>
            <h2>Approve, type one command, go back to work.</h2>
            <p>
              No rebase loop, no broken <code>main</code>, no &quot;who merged that?&quot;. You do not create
              a queue: the first command creates it.
            </p>
          </div>
          <div className="steps">
            <div className="step">
              <span className="n">01</span>
              <h3>Approve</h3>
              <p>A pull request is approved and its checks are green.</p>
              <div className="ui" aria-hidden="true">
                <div className="row ok">
                  <CheckCircleIcon />
                  <span>
                    Approved by <b>marie</b>
                  </span>
                </div>
                <div className="row ok">
                  <CheckCircleIcon />
                  <span>build</span>
                  <span className="r">Successful in 4m</span>
                </div>
                <div className="row ok">
                  <CheckCircleIcon />
                  <span>lint</span>
                  <span className="r">Successful in 38s</span>
                </div>
                <span className="muted">All checks have passed</span>
              </div>
            </div>
            <div className="step">
              <span className="n">02</span>
              <h3>Queue</h3>
              <p>
                Comment <code>/cihub merge</code>. CIHub tests it on top of everything ahead of it, alone or
                in a batch.
              </p>
              <div className="ui" aria-hidden="true">
                <div className="composer">
                  /cihub merge
                  <span className="caret" />
                </div>
                <div className="row">
                  <Mark className="mark" />
                  <span>
                    <b>2nd in line</b> in the queue of <code>main</code>
                  </span>
                </div>
                <span className="muted">
                  Options: <code>--priority=urgent</code>, <code>--no-batch</code>
                </span>
              </div>
            </div>
            <div className="step">
              <span className="n">03</span>
              <h3>Merge</h3>
              <p>
                If that exact combination passes, CIHub merges it. If not, the pull request leaves the queue,
                with the reason on it.
              </p>
              <div className="ui" aria-hidden="true">
                <span className="merged">
                  <MergeIcon />
                  Merged
                </span>
                <div className="row ok">
                  <CheckCircleIcon />
                  <span>CIHub Merge Queue (main)</span>
                </div>
                <span className="muted">
                  Tested with #120 on <code>main</code> at 3f2a1c9
                </span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="block" id="features">
        <div className="wrap">
          <div className="s-head">
            <span className="eyebrow">{"// why CIHub"}</span>
            <h2>A merge queue that you run, and that explains itself.</h2>
          </div>
          <div className="bento">
            <div className="feat w3">
              <h3>Green by design</h3>
              <p>
                What gets tested is exactly what lands. The required checks are read on the target branch,
                never from the pull request, so that a pull request cannot weaken its own gate.
              </p>
              <div className="art green-art" aria-hidden="true">
                <span className="t">main@3f2a1c9</span>
                <span className="op">+</span>
                <span className="t pr">#120</span>
                <span className="op">+</span>
                <span className="t pr">#121</span>
                <span className="op">=</span>
                <span className="eq">
                  <CheckIcon />
                  what lands
                </span>
              </div>
            </div>
            <div className="feat w3">
              <h3>Your CI, unchanged</h3>
              <p>
                CIHub pushes a test branch and waits for the required statuses and check runs. If it reports a
                check, CIHub waits for it.
              </p>
              <div className="art ci-list">
                {["GitHub Actions", "Buildkite", "CircleCI", "Jenkins"].map((ci) => (
                  <span key={ci}>
                    {ci}
                    <CheckIcon />
                  </span>
                ))}
              </div>
            </div>
            <div className="feat w2">
              <h3>Yours to run</h3>
              <p>
                One container and a Postgres. Your code stays on your infrastructure. One instance serves
                several GitHub organizations.
              </p>
              <div className="art infra" aria-hidden="true">
                <div className="box main">
                  <b>cihub</b>
                  <span>container</span>
                </div>
                <span className="wire" />
                <div className="box">
                  <b>Postgres</b>
                  <span>17</span>
                </div>
              </div>
            </div>
            <div className="feat w4">
              <h3>Never a mystery</h3>
              <p>
                Each pull request says where it stands in the queue and what it waits for: on GitHub, in the
                dashboard, in the CLI.
              </p>
              <div className="art why-list" aria-hidden="true">
                <div className="row">
                  <span className="badge hollow">
                    <span className="dot" />
                  </span>
                  <b>#124</b>Update the install guide
                  <span className="why">Waits for 1 of 2 required approvals</span>
                </div>
                <div className="row">
                  <span className="badge wait">
                    <span className="dot" />
                  </span>
                  <b>#121</b>Fix the login redirect
                  <span className="why">3rd in line · waits for room in the window</span>
                </div>
                <div className="row">
                  <span className="badge red">
                    <span className="dot" />
                  </span>
                  <b>#119</b>Move the user settings
                  <span className="why failed">Tests failed: e2e / checkout · retry 1 of 1</span>
                </div>
              </div>
            </div>
            <div className="feat wide">
              <div className="cols">
                <div className="intro">
                  <h3>Quick when it gets busy</h3>
                  <p>
                    Pull requests are tested in parallel or in batches. A failed batch is split to find the
                    cause, and one flaky failure does not kick anyone out.
                  </p>
                </div>
                <div className="batch-art" aria-hidden="true">
                  <div className="lane">
                    <span className="k">Pipelined</span>
                    <div className="runs">
                      <span className="run">
                        <b>#116</b>on main
                      </span>
                      <span className="run">
                        <b>#117</b>on #116
                      </span>
                      <span className="run">
                        <b>#118</b>on #117
                      </span>
                    </div>
                  </div>
                  <div className="lane">
                    <span className="k">Batched</span>
                    <div className="runs">
                      <span className="run">
                        <b>#118 #119 #120 #121</b>in one run
                      </span>
                    </div>
                  </div>
                  <div className="lane">
                    <span className="k">Split</span>
                    <div className="runs">
                      <span className="run ok">
                        <b>#118 #120</b>passed
                      </span>
                      <span className="run split">
                        <b>#119</b>the cause
                      </span>
                      <span className="run ok">
                        <b>#121</b>passed
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="block" id="config">
        <div className="wrap split">
          <div className="s-head">
            <span className="eyebrow">{"// configuration"}</span>
            <h2>A queue works without a file.</h2>
            <p>
              To change its behavior, add <code>.cihub.yaml</code> at the root of the repository. CIHub reads
              it on the head of the target branch, never from a pull request.
            </p>
            <ul className="ticks">
              <li>
                <span>
                  <strong>sequential</strong>, <strong>pipelined</strong> or <strong>batched</strong>: one
                  test at a time, tests in parallel, or several pull requests per test.
                </span>
              </li>
              <li>Conditions to enter the queue and to merge: labels, reviews, checks, files, authors.</li>
              <li>Priority rules: an urgent fix goes first, dependency updates go last.</li>
              <li>A JSON Schema, so that your editor completes and checks each key.</li>
            </ul>
          </div>
          <div className="code">
            <div className="tabs">
              <span className="on">.cihub.yaml</span>
              <CopyButton text={exampleConfig} />
            </div>
            <YamlBlock source={exampleConfig} />
          </div>
        </div>
      </section>

      <section className="block" id="agents">
        <div className="wrap split reverse">
          <div className="term" aria-hidden="true">
            <div className="tb">
              <i />
              <i />
              <i />
              <span>~/acme/web</span>
            </div>
            <pre>
              {agentSession.map(({ kind, text }, index) => (
                <span key={index}>
                  <TerminalLine kind={kind} text={text} />
                  {"\n"}
                </span>
              ))}
            </pre>
          </div>
          <div className="s-head">
            <span className="eyebrow">{"// for coding agents"}</span>
            <h2>Written for people and for agents.</h2>
            <p>An agent that helps you set up a queue finds on this site everything you find, in Markdown.</p>
            <ul className="ticks">
              <li>
                <span>
                  Start at <a href="/llms.txt">/llms.txt</a>, or read every page in{" "}
                  <a href="/llms-full.txt">/llms-full.txt</a>.
                </span>
              </li>
              <li>
                <span>
                  Each page is Markdown at its URL with <code>.md</code>, or with{" "}
                  <code>Accept: text/markdown</code>.
                </span>
              </li>
              <li>
                <span>
                  The JSON Schema of <code>.cihub.yaml</code> and the OpenAPI document of the API.
                </span>
              </li>
            </ul>
            <Link className="link-arrow" href="/docs/agents">
              Use CIHub with a coding agent
            </Link>
          </div>
        </div>
      </section>

      <div className="band" id="compare">
        <div className="wrap in">
          <div>
            <h2>Coming from another merge queue?</h2>
            <p>The comparison shows the ideas that CIHub takes from each one, and where it is different.</p>
            <p className="more">
              <a className="link-arrow" href={`${repositoryUrl}/blob/main/specs/COMPETITION.md`}>
                Read the comparison
              </a>
            </p>
          </div>
          <div className="peers">
            {["GitHub merge queue", "Mergify", "Trunk", "Aviator", "Graphite", "bors"].map((peer) => (
              <span key={peer}>{peer}</span>
            ))}
          </div>
        </div>
      </div>

      <section className="final">
        <div className="wrap">
          <Mark className="mark" />
          <h2>
            Your first merged pull request
            <br />
            in about 30 minutes.
          </h2>
          <p>
            A GitHub App, a Helm chart and a Postgres. Then comment <code>/cihub merge</code>.
          </p>
          <div className="ctas">
            <Link className="btn primary" href="/docs/install">
              Install CIHub
            </Link>
            <a className="btn" href={repositoryUrl}>
              <GitHubIcon />
              Star on GitHub
            </a>
          </div>
        </div>
      </section>
    </main>
  );
}

function TerminalLine({ kind, text }: (typeof agentSession)[number]) {
  switch (kind) {
    case "command":
      return (
        <>
          <span className="p">$ </span>
          {text}
        </>
      );
    case "heading":
      return <span className="h">{text}</span>;
    case "output":
      return <span className="o">{text}</span>;
    case "live":
      return (
        <>
          <span className="g">●</span>
          {text}
        </>
      );
    default:
      return text;
  }
}

/** Colors a YAML file of keys and scalars: comments, keys, numbers and booleans, strings. */
function YamlBlock({ source }: { source: string }) {
  return (
    <pre>
      {source.split("\n").map((line, index) => (
        <span key={index}>
          <span className="ln">{index + 1}</span>
          <YamlLine line={line} />
          {"\n"}
        </span>
      ))}
    </pre>
  );
}

function YamlLine({ line }: { line: string }) {
  if (line.trimStart().startsWith("#")) return <span className="tk-c">{line}</span>;

  const match = /^(\s*(?:- )?)(\w+):(.*)$/.exec(line);
  if (!match) return line;

  const [, indent, key, value = ""] = match;
  const scalar = value.trim();
  const scalarClass = /^(\d+|true|false)$/.test(scalar) ? "tk-n" : "tk-s";

  return (
    <>
      {indent}
      <span className="tk-k">{key}</span>:
      {scalar && (
        <>
          {" "}
          <span className={scalarClass}>{scalar}</span>
        </>
      )}
    </>
  );
}
