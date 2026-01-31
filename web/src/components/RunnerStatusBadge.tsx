import { Badge } from './Badge';

interface RunnerStatusBadgeProps {
    status: 'pending' | 'registered' | 'idle' | 'busy' | 'completed';
    cancelled?: boolean;
}

export function RunnerStatusBadge({ status, cancelled }: RunnerStatusBadgeProps) {
    if (cancelled) {
        return <Badge variant="danger">cancelled</Badge>;
    }

    const variants = {
        pending: 'warning' as const,
        registered: 'info' as const,
        idle: 'default' as const,
        busy: 'success' as const,
        completed: 'default' as const,
    };

    return <Badge variant={variants[status]}>{status}</Badge>;
}
