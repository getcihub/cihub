import { Mark } from "@/components/mark";

/** The queue of a branch as the dashboard shows it, read from the bottom like `git log --graph`. */
export function QueuePreview() {
  return (
    <div className="stage">
      <div className="window" aria-hidden="true">
        <div className="bar">
          <Mark className="mark" />
          <div className="crumbs">
            acme <i>/</i> web <i>/</i> main
          </div>
          <div className="me">
            <span className="avatar">N</span>nlecoy
          </div>
        </div>
        <div className="body">
          <div className="q-head">
            <div>
              <div className="sub">acme/web</div>
              <h3>
                main
                <span className="badge live">
                  <span className="dot" />
                  Running
                </span>
              </h3>
            </div>
            <div className="actions">
              <span className="btn">Pause</span>
              <span className="btn">Drain</span>
              <span className="btn primary">Submit a pull request</span>
            </div>
          </div>
          <div className="stats">
            <Stat label="In the queue" value="5" hint="and 1 waiting to enter" />
            <Stat label="Testing" value="3" hint="pull requests in 2 runs" />
            <Stat label="Merged today" value="14" hint="last one 6 minutes ago" />
            <Stat label="Failed today" value="1" hint="with the reason on it" />
          </div>
          <div className="panel">
            <div className="panel-head">
              Graph
              <span className="right">
                Read from the bottom: each run tests its pull requests on top of the runs below it.
              </span>
            </div>
            <div className="graph">
              <div className="g-row is-label">
                <div className="rail">
                  <span className="ln q down grey dashed" />
                </div>
                <div className="g-body">
                  <span className="g-label">Waiting: conditions, outside the line</span>
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln q full grey dashed" />
                  <span className="node q dashed" />
                </div>
                <div className="g-body">
                  <span className="chip">
                    <b>#124</b>
                    <span>Update the install guide</span>
                    <span className="why">1 of 2 required approvals</span>
                  </span>
                </div>
              </div>
              <div className="g-row is-label">
                <div className="rail">
                  <span className="ln q full grey" />
                </div>
                <div className="g-body">
                  <span className="g-label">In line: waits for room in the window</span>
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln q full grey" />
                  <span className="node q pending" />
                </div>
                <div className="g-body">
                  <span className="chip mine">
                    <b>#121</b>
                    <span>Fix the login redirect</span>
                    <span className="who">nlecoy</span>
                  </span>
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln q full grey" />
                  <span className="node q pending" />
                </div>
                <div className="g-body">
                  <span className="chip">
                    <b>#123</b>
                    <span>Payment webhook retry</span>
                    <span className="prio">high</span>
                    <span className="who">lea</span>
                  </span>
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln q up grey" />
                  <span className="ln q down" />
                  <span className="node q run failing" />
                </div>
                <div className="g-body">
                  <div className="g-run failing">
                    <div className="g-run-head">
                      <span>Batch #118 and #119</span>
                      <span className="state amber">Tests failed · split to find the cause</span>
                    </div>
                    <span className="chip">
                      <b>#119</b>
                      <span>Move the user settings</span>
                      <span className="who">sam</span>
                    </span>
                  </div>
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln q full" />
                  <span className="node q run" />
                </div>
                <div className="g-body">
                  <div className="g-run testing">
                    <div className="g-run-head">
                      <span>
                        #116 and #117 on <span className="mono">main</span> at{" "}
                        <span className="sha">9c1e2a4</span>
                      </span>
                      <span className="state blue">Testing · attempt 1</span>
                    </div>
                    <div className="bar-run">
                      <i />
                    </div>
                    <span className="chip">
                      <b>#116</b>
                      <span>Export invoices as CSV</span>
                      <span className="who">marie</span>
                    </span>
                    <span className="chip">
                      <b>#117</b>
                      <span>Bump serde to 1.0.228</span>
                      <span className="who">renovate[bot]</span>
                    </span>
                  </div>
                </div>
              </div>
              <div className="g-row is-head">
                <div className="rail">
                  <svg className="fork" viewBox="0 0 48 22" preserveAspectRatio="none">
                    <path d="M14 22 C14 8 38 14 38 0" />
                  </svg>
                  <span className="ln t down" />
                  <span className="node t head">
                    <svg viewBox="0 0 12 12">
                      <path
                        d="M2.5 6.2 L5 8.6 L9.5 3.6"
                        fill="none"
                        stroke="#fff"
                        strokeWidth="1.8"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  </span>
                </div>
                <div className="g-body">
                  <span className="ref">main</span>
                  <Commit sha="3f2a1c9" title="Add the audit log (#115)" when="6 min ago" />
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln t full" />
                  <span className="node t commit" />
                </div>
                <div className="g-body">
                  <Commit
                    sha="a81d07e"
                    title="Retry the webhooks of GitHub on a 502 (#114)"
                    when="21 min ago"
                  />
                </div>
              </div>
              <div className="g-row">
                <div className="rail">
                  <span className="ln t full fade" />
                  <span className="node t commit" />
                </div>
                <div className="g-body">
                  <Commit sha="5be90c4" title="Batch #112 and #113" when="38 min ago" />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="gh-comment" aria-hidden="true">
        <div className="h">
          <Mark className="mark" />
          <b>cihub</b>
          <span className="bot">bot</span>
          commented on #121
        </div>
        <div className="c">
          <span className="badge wait">
            <span className="dot" />
            <span>
              <b>3rd in line</b> in the queue of <code>main</code>
            </span>
          </span>
          <span>It waits for room in the window: 2 runs at a time now. It merges in about 33 min.</span>
          <span className="link">See it in the queue →</span>
        </div>
      </div>
      <div className="gh-status" aria-hidden="true">
        <span className="spin" />
        <div className="t">
          <b>CIHub Merge Queue (main)</b>
          <span>Testing with #116 on main at 9c1e2a4</span>
        </div>
        <span className="label">Required</span>
      </div>
    </div>
  );
}

function Stat({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <div className="stat">
      <div className="label">{label}</div>
      <div className="v">{value}</div>
      <div className="h">{hint}</div>
    </div>
  );
}

function Commit({ sha, title, when }: { sha: string; title: string; when: string }) {
  return (
    <div className="g-commit">
      <span className="sha">{sha}</span>
      <span className="t">{title}</span>
      <span className="when">{when}</span>
    </div>
  );
}
