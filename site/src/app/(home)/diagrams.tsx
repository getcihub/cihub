const check = { stroke: "#fff", strokeWidth: 2, strokeLinecap: "round", strokeLinejoin: "round" } as const;

/** Two pull requests, green on their own, merged one after the other: main is red. */
export function WithoutQueueDiagram() {
  return (
    <svg className="diagram" viewBox="0 0 480 170" fill="none" aria-hidden="true">
      <path d="M20 130 H330" style={{ stroke: "var(--primary)" }} strokeWidth="2" />
      <path d="M330 130 H440" style={{ stroke: "var(--destructive)" }} strokeWidth="2" />
      <path
        d="M70 130 C70 40 90 36 130 36 H240 C280 36 290 60 290 130"
        style={{ stroke: "var(--waiting)" }}
        strokeWidth="2"
      />
      <path
        d="M70 130 C70 84 90 82 130 82 H290 C320 82 330 100 330 130"
        style={{ stroke: "var(--waiting)" }}
        strokeWidth="2"
      />
      <circle cx="70" cy="130" r="5" style={{ fill: "var(--primary)" }} />
      <circle cx="180" cy="36" r="9" style={{ fill: "var(--success)" }} />
      <path d="M175.5 36.5 L178.5 39.5 L184.5 33" {...check} />
      <text x="196" y="40" style={{ fill: "var(--secondary-foreground)" }}>
        #1 CI green
      </text>
      <circle cx="200" cy="82" r="9" style={{ fill: "var(--success)" }} />
      <path d="M195.5 82.5 L198.5 85.5 L204.5 79" {...check} />
      <text x="216" y="86" style={{ fill: "var(--secondary-foreground)" }}>
        #2 CI green
      </text>
      <circle cx="290" cy="130" r="5" style={{ fill: "var(--primary)" }} />
      <circle cx="330" cy="130" r="5" style={{ fill: "var(--destructive)" }} />
      <circle cx="440" cy="130" r="12" style={{ fill: "var(--destructive)" }} />
      <path d="M435 125 L445 135 M445 125 L435 135" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" />
      <text x="20" y="158" style={{ fill: "var(--muted-foreground)" }}>
        main
      </text>
      <text x="440" y="162" textAnchor="middle" style={{ fill: "var(--destructive)" }}>
        red
      </text>
      <text x="385" y="120" textAnchor="middle" style={{ fill: "var(--muted-foreground)" }}>
        #1 + #2
      </text>
    </svg>
  );
}

/** The same two pull requests, tested together on top of main before they land. */
export function WithQueueDiagram() {
  return (
    <svg className="diagram" viewBox="0 0 480 170" fill="none" aria-hidden="true">
      <path d="M20 130 H440" style={{ stroke: "var(--primary)" }} strokeWidth="2" />
      <path
        d="M70 130 C70 64 90 60 130 60 H330 C380 60 400 80 400 130"
        style={{ stroke: "var(--primary)" }}
        strokeWidth="2"
      />
      <rect
        x="140.5"
        y="38.5"
        width="200"
        height="43"
        rx="21.5"
        style={{ fill: "var(--primary-muted)", stroke: "var(--primary-border)" }}
        strokeDasharray="4 4"
      />
      <circle
        cx="168"
        cy="60"
        r="6"
        style={{ fill: "var(--card)", stroke: "var(--primary)" }}
        strokeWidth="2"
      />
      <circle
        cx="196"
        cy="60"
        r="6"
        style={{ fill: "var(--card)", stroke: "var(--primary)" }}
        strokeWidth="2"
      />
      <text x="214" y="64" style={{ fill: "var(--primary)" }}>
        main + #1 + #2
      </text>
      <text x="240" y="28" textAnchor="middle" style={{ fill: "var(--muted-foreground)" }}>
        tested together
      </text>
      <circle cx="70" cy="130" r="5" style={{ fill: "var(--primary)" }} />
      <circle cx="400" cy="130" r="5" style={{ fill: "var(--primary)" }} />
      <circle cx="440" cy="130" r="12" style={{ fill: "var(--primary)" }} />
      <path d="M434.5 130.5 L438.5 134.5 L445.5 126.5" {...check} strokeWidth="2.2" />
      <text x="20" y="158" style={{ fill: "var(--muted-foreground)" }}>
        main
      </text>
      <text x="440" y="162" textAnchor="middle" style={{ fill: "var(--primary)" }}>
        green
      </text>
      <text x="360" y="120" textAnchor="middle" style={{ fill: "var(--muted-foreground)" }}>
        merged
      </text>
    </svg>
  );
}
