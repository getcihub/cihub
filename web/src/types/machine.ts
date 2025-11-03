import type { Runner } from './runner'

export const MachineStatusOnline = 'online'
export const MachineStatusOffline = 'offline'
export const MachineStatusUnhealthy = 'unhealthy'
export const MachineStatusPaused = 'paused'

export type MachineStatus = typeof MachineStatusOnline | typeof MachineStatusOffline | typeof MachineStatusUnhealthy | typeof MachineStatusPaused

export interface Machine {
    name: string
    owner: string
    arch: string
    cpu: number
    ram: number
    status: MachineStatus
    created_at: number
    last_seen_at: number
    updated_at: number
    labels?: string[]
    runners?: Runner[]
}
