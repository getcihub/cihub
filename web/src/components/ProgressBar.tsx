interface ProgressBarProps {
    value: number;
    max: number;
    label?: string;
    color?: string;
}

export function ProgressBar({ value, max, label, color = '#f59e0b' }: ProgressBarProps) {
    const percentage = max > 0 ? Math.min((value / max) * 100, 100) : 0;

    return (
        <div className="space-y-1">
            {label && (
                <div className="flex items-center justify-between">
                    <span className="font-mono text-xs text-muted">{label}</span>
                    <span className="font-mono text-xs text-secondary">
                        {value.toFixed(0)}/{max.toFixed(0)}
                    </span>
                </div>
            )}
            <div className="h-1.5 overflow-hidden rounded-full bg-white/5">
                <div
                    className="h-full rounded-full transition-all duration-300"
                    style={{ width: `${percentage}%`, backgroundColor: color }}
                />
            </div>
        </div>
    );
}
