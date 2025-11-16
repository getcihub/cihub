import type { Runner } from './runner';

export const MachineStatusOnline = 'online';
export const MachineStatusOffline = 'offline';
export const MachineStatusUnhealthy = 'unhealthy';
export const MachineStatusPaused = 'paused';

export type MachineStatus =
    | typeof MachineStatusOnline
    | typeof MachineStatusOffline
    | typeof MachineStatusUnhealthy
    | typeof MachineStatusPaused;

export interface Machine {
    name: string;
    owner: string;
    arch: string;
    cpu: number;
    cpu_limit: number;
    cpu_allocated: number;
    ram_available: number;
    ram_limit: number;
    ram_allocated: number;
    ram_total: number;
    status: MachineStatus;
    created_at: number;
    last_seen_at: number;
    updated_at: number;
    labels?: string[];
    runners?: Runner[];
}
