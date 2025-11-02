import { useQuery } from '@tanstack/react-query'
import type { Runner } from '../types/runner'
import type { PaginatedApiResponse } from '../types/api'
import { useInstallation } from './useInstallation'

// Mock runners data for development
const MOCK_RUNNERS: Runner[] = [
    {
        name: 'runner-1',
        machine: 'runner-1',
        id: 123456,
        installation_id: 12345,
        owner: 'myorg',
        status: 'busy',
        arch: 'x86_64',
        cpu: 4,
        ram: 8192,
        group_id: 0,
        labels: ['linux', 'x64', 'self-hosted'],
        cancelled: 0,
        created: Math.floor(Date.now() / 1000) - 86400 * 30,
        accepted: Math.floor(Date.now() / 1000) - 300,
        started: Math.floor(Date.now() / 1000) - 280,
        stopped: 0,
        updated: Math.floor(Date.now() / 1000) - 10,
    },
    {
        name: 'runner-2',
        machine: 'runner-2',
        id: 123457,
        installation_id: 12345,
        owner: 'myorg',
        status: 'idle',
        arch: 'x86_64',
        cpu: 8,
        ram: 16384,
        group_id: 0,
        labels: ['linux', 'x64', 'self-hosted'],
        cancelled: 0,
        created: Math.floor(Date.now() / 1000) - 86400 * 20,
        accepted: Math.floor(Date.now() / 1000) - 600,
        started: 0,
        stopped: 0,
        updated: Math.floor(Date.now() / 1000) - 50,
    },
    {
        name: 'runner-3',
        machine: 'runner-3',
        id: 123458,
        installation_id: 12345,
        owner: 'myorg',
        status: 'completed',
        arch: 'arm64',
        cpu: 4,
        ram: 4096,
        group_id: 0,
        labels: ['linux', 'arm64', 'self-hosted'],
        cancelled: 0,
        created: Math.floor(Date.now() / 1000) - 86400 * 15,
        accepted: Math.floor(Date.now() / 1000) - 7200,
        started: Math.floor(Date.now() / 1000) - 7150,
        stopped: Math.floor(Date.now() / 1000) - 3600,
        updated: Math.floor(Date.now() / 1000) - 3600,
    },
]

export function useRunners() {
    const { selectedInstallation } = useInstallation()

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'runners'],
        queryFn: async () => {
            if (!selectedInstallation) {
                return []
            }

            try {
                const response = await fetch(`/api/installations/${selectedInstallation.login}/runners`)
                if (!response.ok) {
                    throw new Error('Failed to fetch runners')
                }
                const data = (await response.json()) as PaginatedApiResponse<Runner>
                if (data.error) {
                    throw new Error(data.reason || 'Failed to fetch runners')
                }
                return data.data && Array.isArray(data.data) && data.data.length > 0 ? data.data : MOCK_RUNNERS
            } catch (error) {
                console.warn('Failed to fetch runners from API, using mock data:', error)
                return MOCK_RUNNERS
            }
        },
        enabled: !!selectedInstallation,
        retry: false,
    })
}

export function useRunnersByMachine(machineName: string) {
    const { data: runners = [] } = useRunners()
    return runners.filter((r) => r.machine === machineName)
}
