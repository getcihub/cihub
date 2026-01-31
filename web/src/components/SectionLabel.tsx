import { type ReactNode } from 'react';

interface SectionLabelProps {
    children: ReactNode;
    count?: number;
    className?: string;
}

export function SectionLabel({ children, count, className = '' }: SectionLabelProps) {
    return (
        <h3 className={`text-xs font-medium uppercase tracking-wider text-secondary ${className}`}>
            {children}
            {count !== undefined && (
                <span className="text-muted"> ({count.toLocaleString()})</span>
            )}
        </h3>
    );
}
