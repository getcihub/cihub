/** The queue as a line toward main: waiting, testing, one that fails, merged. */
export function Timeline({ className }: { className: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 560 90"
      fill="none"
      fontFamily="Inter, -apple-system, sans-serif"
      fontSize="10"
      fontWeight="600"
      letterSpacing="1"
      aria-hidden="true"
    >
      <path d="M10 46 H200" style={{ stroke: "var(--waiting)" }} strokeWidth="2" strokeDasharray="4 5" />
      <path d="M196 46 H540" style={{ stroke: "var(--primary)" }} strokeWidth="2" />
      <circle
        cx="40"
        cy="46"
        r="6"
        style={{ fill: "var(--muted)", stroke: "var(--waiting)" }}
        strokeWidth="2"
      />
      <circle
        cx="84"
        cy="46"
        r="6"
        style={{ fill: "var(--muted)", stroke: "var(--waiting)" }}
        strokeWidth="2"
      />
      <rect
        x="126.5"
        y="32.5"
        width="91"
        height="27"
        rx="13.5"
        style={{ fill: "var(--primary-muted)", stroke: "var(--primary-border)" }}
        strokeDasharray="4 4"
      />
      {[148, 172, 196].map((x) => (
        <circle
          key={x}
          cx={x}
          cy="46"
          r="5.5"
          style={{ fill: "var(--muted)", stroke: "var(--primary)" }}
          strokeWidth="2"
        />
      ))}
      <path
        d="M196 33 C196 20 210 14 236 14"
        style={{ stroke: "var(--destructive-border)" }}
        strokeWidth="2"
        strokeDasharray="4 4"
        strokeLinecap="round"
        fill="none"
      />
      <circle
        cx="247"
        cy="14"
        r="9"
        style={{ fill: "var(--destructive-muted)", stroke: "var(--destructive)" }}
        strokeWidth="1.5"
      />
      <path
        d="M243.5 10.5 L250.5 17.5 M250.5 10.5 L243.5 17.5"
        style={{ stroke: "var(--destructive)" }}
        strokeWidth="2"
        strokeLinecap="round"
      />
      <text x="263" y="18" style={{ fill: "var(--destructive)" }}>
        FAILED
      </text>
      {[360, 400, 440].map((x) => (
        <circle key={x} cx={x} cy="46" r="5.5" style={{ fill: "var(--primary)" }} />
      ))}
      <circle cx="540" cy="46" r="11" style={{ fill: "var(--primary)" }} />
      <path
        d="M535 46.5 L538.5 50 L545 43.5"
        stroke="#fff"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <g style={{ fill: "var(--muted-foreground)" }} textAnchor="middle">
        <text x="62" y="80">
          WAITING
        </text>
        <text x="172" y="80">
          TESTING
        </text>
        <text x="400" y="80">
          MERGED
        </text>
        <text x="540" y="80">
          MAIN
        </text>
      </g>
    </svg>
  );
}
