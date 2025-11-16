import { useMachines } from './useMachines';

export interface UsageMetrics {
    machines_used: number;
    vcpu_used: number;
    isLoading: boolean;
    error: Error | null;
}

/**
 * Calculate current usage metrics for the selected installation
 * - machines_used: count of machines
 * - vcpu_used: sum of vCPU capacity across all machines
 */
export function useUsageMetrics(): UsageMetrics {
    const { data: machines = [], isLoading, error } = useMachines();

    // Calculate machines count
    const machines_used = machines.length;

    // Calculate total vCPU capacity available from all machines
    const vcpu_used = machines.reduce(
        (total, machine) => total + machine.cpu,
        0,
    );

    return {
        machines_used,
        vcpu_used,
        isLoading,
        error: error instanceof Error ? error : null,
    };
}
