/** The queue toward main breaks: a page could not load. */
export function FailureIllustration() {
  return (
    <svg className="mb-2.5 block h-auto w-60 max-md:w-50" viewBox="0 0 260 64" fill="none" aria-hidden="true">
      <path d="M10 32 H104" style={{ stroke: "var(--primary)" }} strokeWidth="1.5" />
      <circle cx="40" cy="32" r="3.5" style={{ fill: "var(--primary)" }} />
      <circle cx="76" cy="32" r="3.5" style={{ fill: "var(--primary)" }} />
      <circle
        cx="130"
        cy="32"
        r="12"
        style={{ fill: "var(--destructive-muted)", stroke: "var(--destructive)" }}
        strokeWidth="1.5"
      />
      <path
        d="M125 27 L135 37 M135 27 L125 37"
        style={{ stroke: "var(--destructive)" }}
        strokeWidth="2"
        strokeLinecap="round"
      />
      <path d="M156 32 H250" style={{ stroke: "var(--waiting)" }} strokeWidth="1.5" strokeDasharray="4 5" />
    </svg>
  );
}

/** The line leaves toward a question mark: nothing here. */
export function LostIllustration() {
  return (
    <svg className="mb-2.5 block h-auto w-60 max-md:w-50" viewBox="0 0 260 64" fill="none" aria-hidden="true">
      <path
        d="M10 44 H90 Q120 44 130 26 Q140 10 170 14"
        style={{ stroke: "var(--waiting)" }}
        strokeWidth="1.5"
        strokeDasharray="4 5"
      />
      <circle cx="40" cy="44" r="3.5" style={{ fill: "var(--waiting)" }} />
      <circle cx="72" cy="44" r="3.5" style={{ fill: "var(--waiting)" }} />
      <circle cx="188" cy="16" r="11" style={{ stroke: "var(--muted-foreground)" }} strokeWidth="1.5" />
      <text
        x="188"
        y="21"
        textAnchor="middle"
        fontSize="14"
        fontWeight="600"
        style={{ fill: "var(--muted-foreground)" }}
      >
        ?
      </text>
    </svg>
  );
}
