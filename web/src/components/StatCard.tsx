import { cx } from '@/lib/utils';
import type { RemixiconComponentType } from '@remixicon/react';
import { motion } from 'framer-motion';
import type { ReactNode } from 'react';

interface StatCardProps {
    label: string;
    value: string | number;
    subValue?: string;
    icon?: RemixiconComponentType;
    iconColor?: string;
    iconBgColor?: string;
    trend?: {
        value: number;
        direction: 'up' | 'down' | 'neutral';
    };
    progress?: {
        value: number;
        color?: string;
    };
    delay?: number;
    children?: ReactNode;
    className?: string;
}

export function StatCard({
    label,
    value,
    subValue,
    icon: Icon,
    iconColor = 'text-blue-400',
    iconBgColor = 'bg-blue-500/20',
    trend,
    progress,
    delay = 0,
    children,
    className,
}: StatCardProps) {
    const getTrendColor = (direction: 'up' | 'down' | 'neutral') => {
        switch (direction) {
            case 'up':
                return 'text-emerald-400';
            case 'down':
                return 'text-red-400';
            default:
                return 'text-white/40';
        }
    };

    const getTrendIcon = (direction: 'up' | 'down' | 'neutral') => {
        switch (direction) {
            case 'up':
                return '+';
            case 'down':
                return '-';
            default:
                return '';
        }
    };

    return (
        <motion.div
            className={cx(
                'relative overflow-hidden rounded-xl border border-white/10 bg-white/[0.02] p-6',
                className,
            )}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, delay }}
        >
            <div className="flex items-start justify-between">
                <div className="min-w-0 flex-1">
                    <p className="font-mono text-xs uppercase tracking-wider text-white/50">
                        {label}
                    </p>
                    <div className="mt-2 flex items-baseline gap-2">
                        <p className="font-display text-3xl font-light text-white">
                            {value}
                        </p>
                        {trend && (
                            <span
                                className={cx(
                                    'font-mono text-sm',
                                    getTrendColor(trend.direction),
                                )}
                            >
                                {getTrendIcon(trend.direction)}
                                {Math.abs(trend.value)}%
                            </span>
                        )}
                    </div>
                    {subValue && (
                        <p className="mt-1 font-mono text-xs text-white/40">
                            {subValue}
                        </p>
                    )}
                </div>
                {Icon && (
                    <div className={cx('rounded-lg p-3', iconBgColor)}>
                        <Icon className={cx('size-6', iconColor)} aria-hidden="true" />
                    </div>
                )}
            </div>

            {progress && (
                <div className="mt-4">
                    <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
                        <motion.div
                            className={cx(
                                'h-full rounded-full',
                                progress.color || 'bg-blue-500',
                            )}
                            initial={{ width: 0 }}
                            animate={{ width: `${Math.min(progress.value, 100)}%` }}
                            transition={{ duration: 0.8, delay: delay + 0.2 }}
                        />
                    </div>
                </div>
            )}

            {children}
        </motion.div>
    );
}
