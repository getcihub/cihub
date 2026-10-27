import type { ReactNode } from "react";

import { Mark } from "@cihub/ui";

// The illustrations of the documentation. They draw with the tokens of global.css, so that they follow the theme.

type Tone = "neutral" | "primary" | "success" | "warning" | "destructive" | "dashed";

const tones: Record<Tone, { fill: string; stroke: string; text: string }> = {
  neutral: { fill: "var(--card)", stroke: "var(--border)", text: "var(--foreground)" },
  primary: { fill: "var(--primary-muted)", stroke: "var(--primary-border)", text: "var(--primary)" },
  success: { fill: "var(--success-muted)", stroke: "var(--success)", text: "var(--success)" },
  warning: { fill: "var(--warning-muted)", stroke: "var(--warning-border)", text: "var(--warning)" },
  destructive: {
    fill: "var(--destructive-muted)",
    stroke: "var(--destructive-border)",
    text: "var(--destructive)",
  },
  dashed: { fill: "var(--card)", stroke: "var(--waiting)", text: "var(--muted-foreground)" },
};

const muted = { fill: "var(--muted-foreground)" };
const mono = "var(--font-mono)";

function Figure({ caption, children }: { caption?: ReactNode; children: ReactNode }) {
  return (
    <figure className="not-prose bg-fd-card my-6 rounded-lg border p-4">
      {children}
      {caption && (
        <figcaption className="text-fd-muted-foreground mt-3 text-center text-sm">{caption}</figcaption>
      )}
    </figure>
  );
}

function Svg({
  width,
  height,
  label,
  children,
}: {
  width: number;
  height: number;
  label: string;
  children: ReactNode;
}) {
  return (
    <svg
      viewBox={`0 0 ${String(width)} ${String(height)}`}
      role="img"
      aria-label={label}
      className="h-auto w-full"
      fontSize="12"
    >
      {children}
    </svg>
  );
}

/** An arrowhead per diagram: two diagrams on one page must not share an id. */
function Arrowhead({ id }: { id: string }) {
  return (
    <defs>
      <marker
        id={id}
        viewBox="0 0 10 10"
        refX="9"
        refY="5"
        markerWidth="7"
        markerHeight="7"
        orient="auto-start-reverse"
      >
        <path d="M0 0 L10 5 L0 10 z" style={muted} />
      </marker>
    </defs>
  );
}

function Arrow({
  d,
  marker,
  both = false,
  dashed = false,
}: {
  d: string;
  marker: string;
  both?: boolean;
  dashed?: boolean;
}) {
  return (
    <path
      d={d}
      fill="none"
      style={{ stroke: "var(--muted-foreground)" }}
      strokeWidth="1.5"
      strokeDasharray={dashed ? "4 4" : undefined}
      markerEnd={`url(#${marker})`}
      markerStart={both ? `url(#${marker})` : undefined}
    />
  );
}

function Box({
  x,
  y,
  width,
  height,
  title,
  lines = [],
  tone = "neutral",
}: {
  x: number;
  y: number;
  width: number;
  height: number;
  title: string;
  lines?: string[];
  tone?: Tone;
}) {
  const color = tones[tone];
  const top = y + height / 2 - (lines.length * 15) / 2 + 4;

  return (
    <g>
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        rx="6"
        style={{ fill: color.fill, stroke: color.stroke }}
        strokeDasharray={tone === "dashed" ? "4 4" : undefined}
      />
      <text
        x={x + width / 2}
        y={top}
        textAnchor="middle"
        fontWeight="600"
        fontSize="13"
        style={{ fill: color.text }}
      >
        {title}
      </text>
      {lines.map((line, index) => (
        <text
          key={line}
          x={x + width / 2}
          y={top + 16 + index * 15}
          textAnchor="middle"
          fontSize="11"
          style={muted}
        >
          {line}
        </text>
      ))}
    </g>
  );
}

function Label({
  x,
  y,
  children,
  anchor = "middle",
}: {
  x: number;
  y: number;
  children: string;
  anchor?: "start" | "middle" | "end";
}) {
  return (
    <text x={x} y={y} textAnchor={anchor} fontSize="11" style={muted}>
      {children}
    </text>
  );
}

function Check({ x, y, tone = "success" }: { x: number; y: number; tone?: "success" | "destructive" }) {
  const color = tone === "success" ? "var(--success)" : "var(--destructive)";
  const mark = tone === "success" ? "M-3 0 L-1 2.5 L3.5 -2.5" : "M-2.5 -2.5 L2.5 2.5 M2.5 -2.5 L-2.5 2.5";

  return (
    <g transform={`translate(${String(x)} ${String(y)})`}>
      <circle r="7" style={{ fill: color }} />
      <path
        d={mark}
        fill="none"
        stroke="#fff"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </g>
  );
}

/** What an instance is made of, and who talks to it. */
export function ArchitectureDiagram() {
  return (
    <Figure caption="One container and a Postgres. GitHub sends the events of your repositories; your CI does not change.">
      <Svg
        width={720}
        height={300}
        label="GitHub sends webhooks to CIHub, which writes to Postgres and acts on GitHub; your CI reports its checks to GitHub"
      >
        <Arrowhead id="architecture-arrow" />
        <Box x={330} y={10} width={180} height={44} title="Your team" lines={["dashboard, CLI, API"]} />
        <Box
          x={20}
          y={100}
          width={170}
          height={96}
          title="GitHub"
          lines={["your repositories", "the App of your CIHub"]}
        />
        <Box
          x={330}
          y={100}
          width={180}
          height={96}
          title="CIHub"
          lines={["one container", "web · scheduler · worker"]}
          tone="primary"
        />
        <Box x={580} y={118} width={120} height={60} title="Postgres 17" lines={["the queues"]} />
        <Box x={20} y={236} width={170} height={50} title="Your CI" lines={["Actions, Buildkite, …"]} />
        <Arrow d="M420 54 V96" marker="architecture-arrow" />
        <Label x={428} y={80} anchor="start">
          sign in, submit, follow
        </Label>
        <Arrow d="M190 126 H326" marker="architecture-arrow" />
        <Label x={260} y={118}>
          webhooks
        </Label>
        <Arrow d="M330 170 H194" marker="architecture-arrow" />
        <Label x={260} y={186}>
          test branches,
        </Label>
        <Label x={260} y={200}>
          merges, comments
        </Label>
        <Arrow d="M510 148 H576" marker="architecture-arrow" both />
        <Arrow d="M105 196 V232" marker="architecture-arrow" both />
        <Label x={114} y={222} anchor="start">
          builds the test commit, reports its checks
        </Label>
      </Svg>
    </Figure>
  );
}

/** The commit that a test holds: the target branch, the pull requests ahead, the pull request. */
export function TestCommitDiagram() {
  return (
    <Figure caption="The test of #121 holds the head of main, the pull requests ahead of it in the line, and #121. What passes is exactly what lands.">
      <Svg
        width={720}
        height={190}
        label="A test commit built from main, #118, #119 and #121, on the branch cihub-merge/pr-121, tested by your CI"
      >
        <Arrowhead id="test-commit-arrow" />
        <path d="M20 140 H230" style={{ stroke: "var(--primary)" }} strokeWidth="2" />
        {[60, 120, 180].map((x) => (
          <circle key={x} cx={x} cy={140} r="5" style={{ fill: "var(--primary)" }} />
        ))}
        <circle
          cx={230}
          cy={140}
          r="8"
          style={{ fill: "var(--card)", stroke: "var(--primary)" }}
          strokeWidth="2.5"
        />
        <text x={20} y={170} fontSize="11" fontFamily={mono} style={muted}>
          main
        </text>
        <Label x={230} y={170}>
          head of main
        </Label>
        <path
          d="M230 132 C230 90 250 70 290 70 H556"
          fill="none"
          style={{ stroke: "var(--primary-border)" }}
          strokeWidth="2"
          strokeDasharray="5 4"
        />
        <Label x={360} y={30}>
          ahead in the line
        </Label>
        <path d="M310 38 V42 H410 V38" fill="none" style={{ stroke: "var(--waiting)" }} />
        {[
          { x: 320, name: "#118" },
          { x: 400, name: "#119" },
        ].map(({ x, name }) => (
          <g key={name}>
            <circle
              cx={x}
              cy={70}
              r="7"
              style={{ fill: "var(--card)", stroke: "var(--primary)" }}
              strokeWidth="2"
            />
            <text x={x} y={58} textAnchor="middle" fontWeight="600" style={{ fill: "var(--foreground)" }}>
              {name}
            </text>
          </g>
        ))}
        <circle cx={480} cy={70} r="8" style={{ fill: "var(--primary)" }} />
        <text x={480} y={56} textAnchor="middle" fontWeight="600" style={{ fill: "var(--primary)" }}>
          #121
        </text>
        <Label x={480} y={30}>
          this PR
        </Label>
        <rect x={552} y={62} width={16} height={16} rx="3" style={{ fill: "var(--primary)" }} />
        <text x={560} y={104} textAnchor="middle" fontWeight="600" style={{ fill: "var(--foreground)" }}>
          test commit
        </text>
        <text x={560} y={120} textAnchor="middle" fontSize="10.5" fontFamily={mono} style={muted}>
          cihub-merge/pr-121/run_…
        </text>
        <Arrow d="M572 70 H612" marker="test-commit-arrow" />
        <rect
          x={616}
          y={36}
          width={94}
          height={70}
          rx="6"
          style={{ fill: "var(--card)", stroke: "var(--border)" }}
        />
        <text
          x={663}
          y={56}
          textAnchor="middle"
          fontWeight="600"
          fontSize="13"
          style={{ fill: "var(--foreground)" }}
        >
          Your CI
        </text>
        <Check x={636} y={74} />
        <text x={648} y={78} fontSize="11" fontFamily={mono} style={{ fill: "var(--foreground)" }}>
          build
        </text>
        <Check x={636} y={93} />
        <text x={648} y={97} fontSize="11" fontFamily={mono} style={{ fill: "var(--foreground)" }}>
          test
        </text>
      </Svg>
    </Figure>
  );
}

function State({
  x,
  y,
  title,
  code,
  tone = "neutral",
}: {
  x: number;
  y: number;
  title: string;
  code: string;
  tone?: Tone;
}) {
  const color = tones[tone];

  return (
    <g>
      <rect
        x={x - 65}
        y={y - 20}
        width={130}
        height={40}
        rx="20"
        style={{ fill: color.fill, stroke: color.stroke }}
        strokeDasharray={tone === "dashed" ? "4 4" : undefined}
      />
      <text x={x} y={y - 2} textAnchor="middle" fontWeight="600" fontSize="13" style={{ fill: color.text }}>
        {title}
      </text>
      <text x={x} y={y + 12} textAnchor="middle" fontSize="10" fontFamily={mono} style={muted}>
        {code}
      </text>
    </g>
  );
}

/** The states of a pull request in the queue, and what moves it from one to the next. */
export function StatesDiagram() {
  return (
    <Figure caption="The usual path, left to right. A failed test waits for the pull requests ahead before it decides whose fault it is.">
      <Svg
        width={860}
        height={250}
        label="States: waiting, queued, testing, tests passed, merged; from testing, tests failed, then failed or queued again; cancelled from any state before merged"
      >
        <Arrowhead id="states-arrow" />
        <State x={80} y={80} title="Waiting" code="not_ready" />
        <State x={250} y={80} title="Queued" code="pending" />
        <State x={420} y={80} title="Testing" code="testing" tone="primary" />
        <State x={590} y={80} title="Tests passed" code="tests_passed" tone="primary" />
        <State x={760} y={80} title="Merged" code="merged" tone="success" />
        <State x={420} y={190} title="Tests failed" code="pending_failure" tone="warning" />
        <State x={590} y={190} title="Failed" code="failed" tone="destructive" />
        <State x={80} y={190} title="Cancelled" code="cancelled" tone="dashed" />
        {[145, 315, 485, 655].map((x) => (
          <Arrow key={x} d={`M${String(x)} 80 H${String(x + 38)}`} marker="states-arrow" />
        ))}
        <Label x={165} y={48}>
          meets the rules
        </Label>
        <Label x={335} y={48}>
          room in the window
        </Label>
        <Label x={505} y={48}>
          checks pass
        </Label>
        <Label x={675} y={48}>
          PRs ahead merged
        </Label>
        <Arrow d="M420 100 V168" marker="states-arrow" />
        <Label x={428} y={138} anchor="start">
          a required check fails
        </Label>
        <Arrow d="M485 190 H523" marker="states-arrow" />
        <Label x={504} y={228}>
          its own fault
        </Label>
        <Arrow d="M355 190 H250 V102" marker="states-arrow" />
        <Label x={258} y={160} anchor="start">
          a PR ahead failed
        </Label>
        <Label x={80} y={228}>
          a push, a close, /cihub cancel…
        </Label>
        <Label x={80} y={242}>
          from any state before Merged
        </Label>
      </Svg>
    </Figure>
  );
}

function Run({
  x,
  y,
  width,
  height = 26,
  text,
  tone = "primary",
}: {
  x: number;
  y: number;
  width: number;
  height?: number;
  text: string;
  tone?: Tone;
}) {
  const color = tones[tone];

  return (
    <g>
      <rect
        x={x}
        y={y}
        width={width}
        height={height}
        rx="4"
        style={{ fill: color.fill, stroke: color.stroke }}
      />
      <text x={x + 8} y={y + height / 2 + 4} fontSize="11" style={{ fill: "var(--foreground)" }}>
        {text}
      </text>
    </g>
  );
}

/** What each strategy has merged after one run of the CI. */
export function StrategiesDiagram() {
  return (
    <Figure caption="The same CI, the same pull requests. After one run of the CI, sequential merged one pull request, pipelined three, batched four.">
      <Svg
        width={720}
        height={260}
        label="Sequential tests one pull request after the other; pipelined tests three at the same time, each on top of the ones ahead; batched tests four in one run"
      >
        <Arrowhead id="strategies-arrow" />
        <path d="M345 26 V228" style={{ stroke: "var(--waiting)" }} strokeDasharray="3 4" />
        <Label x={345} y={18}>
          one run of the CI
        </Label>
        <text x={20} y={64} fontFamily={mono} fontSize="12" style={{ fill: "var(--foreground)" }}>
          sequential
        </text>
        <Run x={140} y={47} width={170} text="#1 on main" />
        <Run x={330} y={47} width={170} text="#2 on main" />
        <Run x={520} y={47} width={170} text="#3 on main" />
        {[310, 500, 690].map((x) => (
          <Check key={x} x={x + 8} y={60} />
        ))}
        <text x={20} y={134} fontFamily={mono} fontSize="12" style={{ fill: "var(--foreground)" }}>
          pipelined
        </text>
        <Run x={140} y={100} width={170} height={20} text="#1 on main" />
        <Run x={150} y={124} width={170} height={20} text="#2 on main + #1" />
        <Run x={160} y={148} width={170} height={20} text="#3 on main + #1 + #2" />
        <Check x={318} y={110} />
        <Check x={328} y={134} />
        <Check x={338} y={158} />
        <text x={20} y={204} fontFamily={mono} fontSize="12" style={{ fill: "var(--foreground)" }}>
          batched
        </text>
        <Run x={140} y={187} width={170} text="#1 #2 #3 #4 on main" />
        <Run x={330} y={187} width={170} text="#5 #6 #7 #8 on #1 – #4" />
        <Check x={318} y={200} />
        <Arrow d="M140 240 H700" marker="strategies-arrow" />
        <Label x={700} y={255} anchor="end">
          time
        </Label>
      </Svg>
    </Figure>
  );
}

/** How a failed batch of 8 finds its cause in 3 rounds. */
export function BisectionDiagram() {
  const link = (x1: number, y1: number, x2: number, y2: number) => (
    <path
      d={`M${String(x1)} ${String(y1)} V${String((y1 + y2) / 2)} H${String(x2)} V${String(y2)}`}
      fill="none"
      style={{ stroke: "var(--waiting)" }}
    />
  );

  return (
    <Figure caption="A batch of 8 fails: the queue splits it until it finds the cause. The halves that pass merge; only #8 leaves the queue.">
      <Svg
        width={720}
        height={290}
        label="A failed batch of 8 is split in 4 and 4, then 2 and 2, then 1 and 1, which finds #8"
      >
        <Label x={20} y={41} anchor="start">
          the batch
        </Label>
        <Label x={20} y={111} anchor="start">
          round 1
        </Label>
        <Label x={20} y={181} anchor="start">
          round 2
        </Label>
        <Label x={20} y={251} anchor="start">
          round 3
        </Label>
        <Run
          x={200}
          y={22}
          width={320}
          height={30}
          text="#1 #2 #3 #4 #5 #6 #7 #8  failed"
          tone="destructive"
        />
        {link(360, 52, 230, 92)}
        {link(360, 52, 490, 92)}
        <Run x={120} y={92} width={220} height={30} text="#1 #2 #3 #4  passed" tone="success" />
        <Run x={380} y={92} width={220} height={30} text="#5 #6 #7 #8  failed" tone="destructive" />
        {link(490, 122, 430, 162)}
        {link(490, 122, 550, 162)}
        <Run x={380} y={162} width={100} height={30} text="#5 #6  passed" tone="success" />
        <Run x={500} y={162} width={100} height={30} text="#7 #8  failed" tone="destructive" />
        {link(550, 192, 522, 232)}
        {link(550, 192, 578, 232)}
        <Run x={500} y={232} width={45} height={30} text="#7" tone="success" />
        <Run x={555} y={232} width={45} height={30} text="#8" tone="destructive" />
        <text x={612} y={251} fontWeight="600" style={{ fill: "var(--destructive)" }}>
          the cause
        </text>
      </Svg>
    </Figure>
  );
}

/** Who can update the target branch once the rulesets are in place. */
export function RulesetsDiagram() {
  return (
    <Figure caption="The ruleset that restricts updates lets only the App of CIHub, in the mode Exempt, write the target branch.">
      <Svg
        width={680}
        height={180}
        label="A merge from GitHub stops at the ruleset; CIHub, exempt, merges after a test"
      >
        <Arrowhead id="rulesets-arrow" />
        <Box
          x={20}
          y={30}
          width={190}
          height={46}
          title="Merge button, git push"
          lines={["a person, a bot"]}
        />
        <Box
          x={20}
          y={110}
          width={190}
          height={46}
          title="CIHub"
          lines={["after a test that passed"]}
          tone="primary"
        />
        <rect x={340} y={16} width={8} height={152} rx="3" style={{ fill: "var(--waiting)" }} />
        <Label x={344} y={12}>
          Restrict updates
        </Label>
        <Arrow d="M210 53 H330" marker="rulesets-arrow" />
        <Check x={318} y={53} tone="destructive" />
        <Arrow d="M210 133 H496" marker="rulesets-arrow" />
        <Check x={374} y={133} />
        <Label x={420} y={124} anchor="start">
          Exempt
        </Label>
        <Box
          x={500}
          y={92}
          width={160}
          height={82}
          title="main"
          lines={["the target branch", "stays green"]}
          tone="success"
        />
      </Svg>
    </Figure>
  );
}

/** The three addresses of the App, and what GitHub sends to each one. */
export function GithubAppUrlsDiagram() {
  const rows = [
    { y: 85, field: "Webhook URL", path: "/webhooks/github", purpose: "the events of your repositories" },
    {
      y: 130,
      field: "Callback URL",
      path: "/auth/github/callback",
      purpose: "a person back from the sign-in",
    },
    {
      y: 175,
      field: "Setup URL",
      path: "/auth/github/installed",
      purpose: "a person back from an installation",
    },
  ];

  return (
    <Figure caption="Three addresses of the App, three paths of CIHub on the same host. GitHub calls the first one; it sends people to the other two.">
      <Svg
        width={720}
        height={225}
        label="The Webhook URL, the Callback URL and the Setup URL of the App point to three paths of CIHub"
      >
        <Arrowhead id="urls-arrow" />
        <text x={20} y={40} fontWeight="600" fontSize="13" style={{ fill: "var(--foreground)" }}>
          Your GitHub App
        </text>
        <text x={480} y={40} fontWeight="600" fontSize="13" style={{ fill: "var(--primary)" }}>
          CIHub
        </text>
        <text x={530} y={40} fontSize="11" fontFamily={mono} style={muted}>
          https://cihub.example.com
        </text>
        <rect
          x={20}
          y={55}
          width={180}
          height={150}
          rx="6"
          style={{ fill: "var(--card)", stroke: "var(--border)" }}
        />
        <rect
          x={480}
          y={55}
          width={220}
          height={150}
          rx="6"
          style={{ fill: "var(--primary-muted)", stroke: "var(--primary-border)" }}
        />
        {rows.map(({ y, field, path, purpose }) => (
          <g key={field}>
            <text x={36} y={y + 4} fontWeight="600" style={{ fill: "var(--foreground)" }}>
              {field}
            </text>
            <Arrow d={`M200 ${String(y)} H476`} marker="urls-arrow" />
            <Label x={338} y={y + 16}>
              {purpose}
            </Label>
            <text x={496} y={y + 4} fontSize="11.5" fontFamily={mono} style={{ fill: "var(--primary)" }}>
              {path}
            </text>
          </g>
        ))}
      </Svg>
    </Figure>
  );
}

/** Where each key of the App comes from, and the variable of the server that takes it. */
export function GithubAppKeysDiagram() {
  const rows = [
    { key: "App ID", from: "General, at the top", variable: "CIHUB_GITHUB_APP_ID" },
    { key: "Client ID", from: "General, at the top", variable: "CIHUB_GITHUB_CLIENT_ID" },
    { key: "Client secret", from: "Generate a new client secret", variable: "CIHUB_GITHUB_CLIENT_SECRET" },
    { key: "Private key", from: "Generate a private key (.pem)", variable: "CIHUB_GITHUB_PRIVATE_KEY_FILE" },
    { key: "Webhook secret", from: "openssl rand -hex 32", variable: "CIHUB_GITHUB_WEBHOOK_SECRET" },
  ];

  return (
    <Figure caption="By hand, you copy five values. cihub app create writes them for you in github-app.env and github-app.private-key.pem.">
      <Svg
        width={720}
        height={250}
        label="The App ID, the Client ID, the client secret, the private key and the webhook secret go to five variables of the server"
      >
        <Arrowhead id="keys-arrow" />
        <Label x={20} y={22} anchor="start">
          on the page of the App
        </Label>
        <Label x={440} y={22} anchor="start">
          the server of CIHub
        </Label>
        {rows.map(({ key, from, variable }, index) => {
          const y = 34 + index * 42;
          return (
            <g key={key}>
              <rect
                x={20}
                y={y}
                width={250}
                height={32}
                rx="5"
                style={{ fill: "var(--card)", stroke: "var(--border)" }}
              />
              <text x={32} y={y + 20} fontWeight="600" style={{ fill: "var(--foreground)" }}>
                {key}
              </text>
              <text x={258} y={y + 20} textAnchor="end" fontSize="10.5" style={muted}>
                {from}
              </text>
              <Arrow d={`M270 ${String(y + 16)} H436`} marker="keys-arrow" />
              <rect
                x={440}
                y={y}
                width={260}
                height={32}
                rx="5"
                style={{ fill: "var(--primary-muted)", stroke: "var(--primary-border)" }}
              />
              <text x={452} y={y + 20} fontSize="11" fontFamily={mono} style={{ fill: "var(--primary)" }}>
                {variable}
              </text>
            </g>
          );
        })}
      </Svg>
    </Figure>
  );
}

/** The comment and the statuses of CIHub on a pull request under test, as GitHub shows them. */
export function PrCommentPreview() {
  return (
    <figure className="not-prose my-6 grid gap-3">
      <div className="bg-fd-card overflow-hidden rounded-lg border text-sm">
        <div className="bg-fd-muted text-fd-muted-foreground flex items-center gap-2 border-b px-3 py-2">
          <Mark className="size-5" />
          <b className="text-fd-foreground">acme-cihub</b>
          <span className="rounded-full border px-1.5 text-xs">bot</span>
          commented
        </div>
        <div className="grid gap-3 px-4 py-3">
          <p>
            🧪 <b>Testing</b> · position 2 in the queue for <code>main</code>
          </p>
          <p>Tested on top of #118, #119 in #130.</p>
          <p>
            <span className="text-fd-primary underline">Open in CIHub</span>
          </p>
          <p className="text-fd-muted-foreground">▸ History (3)</p>
          <p className="text-fd-muted-foreground text-xs">
            <code>/cihub restart</code> to test again · <code>/cihub cancel</code> to leave the queue
          </p>
        </div>
      </div>
      <div className="bg-fd-card divide-y rounded-lg border text-sm">
        <div className="flex items-center gap-3 px-3 py-2">
          <span className="size-3 rounded-full" style={{ background: "var(--warning)" }} />
          <b>CIHub Merge Queue (main)</b>
          <span className="text-fd-muted-foreground">Testing, position 2</span>
          <span className="text-fd-primary ms-auto">Details</span>
        </div>
        <div className="flex items-center gap-3 px-3 py-2">
          <span className="size-3 rounded-full" style={{ background: "var(--success)" }} />
          <b>CIHub Conditions (main)</b>
          <span className="text-fd-muted-foreground">Ready for the queue</span>
        </div>
      </div>
      <figcaption className="text-fd-muted-foreground text-center text-sm">
        The comment of CIHub, edited at each change, and its two statuses in the checks of the pull request.
      </figcaption>
    </figure>
  );
}
