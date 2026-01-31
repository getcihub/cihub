import { type ReactNode } from 'react';

interface TableProps {
    children: ReactNode;
}

export function Table({ children }: TableProps) {
    return (
        <div className="overflow-x-auto">
            <table className="w-full">{children}</table>
        </div>
    );
}

export function TableHeader({ children }: { children: ReactNode }) {
    return <thead className="border-b border-white/5">{children}</thead>;
}

export function TableBody({ children }: { children: ReactNode }) {
    return <tbody className="divide-y divide-white/5">{children}</tbody>;
}

export function TableRow({
    children,
    className = '',
}: {
    children: ReactNode;
    className?: string;
}) {
    return (
        <tr className={`transition-colors hover:bg-white/[0.02] ${className}`}>
            {children}
        </tr>
    );
}

export function TableHead({
    children,
    className = '',
}: {
    children: ReactNode;
    className?: string;
}) {
    return (
        <th
            className={`px-4 py-3 text-left font-mono text-xs font-medium uppercase tracking-wider text-secondary ${className}`}
        >
            {children}
        </th>
    );
}

export function TableCell({
    children,
    className = '',
}: {
    children: ReactNode;
    className?: string;
}) {
    return (
        <td className={`px-4 py-3 font-mono text-sm text-white ${className}`}>
            {children}
        </td>
    );
}
