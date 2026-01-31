import { Badge } from './Badge';

interface MachineStatusBadgeProps {
    status: 'online' | 'offline' | 'paused';
}

export function MachineStatusBadge({ status }: MachineStatusBadgeProps) {
    const variants = {
        online: 'success' as const,
        offline: 'default' as const,
        paused: 'warning' as const,
    };

    return <Badge variant={variants[status]}>{status}</Badge>;
}
