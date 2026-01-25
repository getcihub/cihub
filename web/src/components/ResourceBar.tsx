import { cx } from '@/lib/utils';
import type { RemixiconComponentType } from '@remixicon/react';
import { motion } from 'framer-motion';

interface ResourceBarProps {
    label: string;
    icon?: RemixiconComponentType;
    iconColor?: string;
    allocated: number;
    limit: number;
    total?: number;
    unit?: string;
    barColor?: string;
    showDetails?: boolean;
    size?: 'sm' | 'md' | 'lg';
    delay?: number;
}

export function ResourceBar({
    label,
    icon: Icon,
    iconColor = 'text-white/50',
    allocated,
    limit,
    total,
    unit = '',
    barColor = 'bg-blue-500',
    showDetails = false,
    size = 'md',
    delay = 0,
}: ResourceBarProps) {
    const effectiveLimit = limit > 0 ? limit : total || 0;
    const percentage =
        effectiveLimit > 0 ? Math.round((allocated / effectiveLimit) * 100) : 0;
    const available = effectiveLimit - allocated;

    const getBarHeight = () => {
        switch (size) {
            case 'sm':
                return 'h-1';
            case 'lg':
                return 'h-2.5';
            default:
                return 'h-1.5';
        }
    };

    const getTextSize = () => {
        switch (size) {
            case 'sm':
                return 'text-[10px]';
            case 'lg':
                return 'text-sm';
            default:
                return 'text-xs';
        }
    };

    const getPercentageColor = () => {
        if (percentage >= 90) return 'text-red-400';
        if (percentage >= 75) return 'text-amber-400';
        return 'text-white';
    };

    return (
        <div className="space-y-2">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                    {Icon && (
                        <Icon
                            className={cx('size-4', iconColor)}
                            aria-hidden="true"
                        />
                    )}
                    <span className={cx('font-mono text-white/50', getTextSize())}>
                        {label}
                    </span>
                </div>
                <div className="flex items-center gap-2">
                    <span className={cx('font-mono', getTextSize(), getPercentageColor())}>
                        {percentage}%
                    </span>
                    <span className={cx('font-mono text-white/40', getTextSize())}>
                        {allocated}
                        {unit} / {effectiveLimit}
                        {unit}
                    </span>
                </div>
            </div>

            <div
                className={cx(
                    'overflow-hidden rounded-full bg-white/10',
                    getBarHeight(),
                )}
            >
                <motion.div
                    className={cx('h-full rounded-full', barColor)}
                    initial={{ width: 0 }}
                    animate={{ width: `${Math.min(percentage, 100)}%` }}
                    transition={{ duration: 0.6, delay }}
                />
            </div>

            {showDetails && effectiveLimit > 0 && (
                <motion.div
                    className="grid grid-cols-2 gap-4 pt-2 sm:grid-cols-4"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ duration: 0.3, delay: delay + 0.3 }}
                >
                    <div>
                        <p className="font-mono text-[10px] text-white/40">Allocated</p>
                        <p className="font-mono text-xs text-white/70">
                            {allocated}
                            {unit}
                        </p>
                    </div>
                    <div>
                        <p className="font-mono text-[10px] text-white/40">Available</p>
                        <p className="font-mono text-xs text-white/70">
                            {available}
                            {unit}
                        </p>
                    </div>
                    {limit > 0 && (
                        <div>
                            <p className="font-mono text-[10px] text-white/40">Limit</p>
                            <p className="font-mono text-xs text-white/70">
                                {limit}
                                {unit}
                            </p>
                        </div>
                    )}
                    {total && total > 0 && (
                        <div>
                            <p className="font-mono text-[10px] text-white/40">Total</p>
                            <p className="font-mono text-xs text-white/70">
                                {total}
                                {unit}
                            </p>
                        </div>
                    )}
                </motion.div>
            )}
        </div>
    );
}
