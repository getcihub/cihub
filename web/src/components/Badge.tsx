import { type ReactNode } from 'react';

interface BadgeProps {
    children: ReactNode;
    variant?: 'default' | 'success' | 'warning' | 'danger' | 'info';
    className?: string;
}

export function Badge({ children, variant = 'default', className = '' }: BadgeProps) {
    const variants = {
        default: 'bg-white/5 text-white/70 border-white/10',
        success: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
        warning: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
        danger: 'bg-red-500/10 text-red-400 border-red-500/20',
        info: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/20',
    };

    return (
        <span
            className={`inline-flex items-center gap-1 rounded-md border px-2 py-0.5 font-mono text-xs ${variants[variant]} ${className}`}
        >
            {children}
        </span>
    );
}
