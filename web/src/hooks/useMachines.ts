import { useQuery } from '@tanstack/react-query'
import type { Machine } from '../types/machine'
import type { PaginatedApiResponse } from '../types/api'
import { useInstallation } from './useInstallation'

// Mock machines data for development
const MOCK_MACHINES: Machine[] = [
    {
        name: 'cihub-runner-01.eu.rbx.cihub.io',
        owner: 'myorg',
        arch: 'x86_64',
        cpu: 4,
        ram: 8192, // 8GB in MB
        status: 'online',
        created_at: Math.floor(Date.now() / 1000) - 86400 * 30, // 30 days ago
        last_seen_at: Math.floor(Date.now() / 1000) - 300, // 5 minutes ago
        updated_at: Math.floor(Date.now() / 1000) - 3600, // 1 hour ago
        labels: ['linux', 'docker', 'x86_64'],
    },
    {
        name: 'cihub-runner-02.eu.gra.cihub.io',
        owner: 'myorg',
        arch: 'x86_64',
        cpu: 8,
        ram: 16384, // 16GB in MB
        status: 'online',
        created_at: Math.floor(Date.now() / 1000) - 86400 * 20, // 20 days ago
        last_seen_at: Math.floor(Date.now() / 1000) - 600, // 10 minutes ago
        updated_at: Math.floor(Date.now() / 1000) - 7200, // 2 hours ago
        labels: ['linux', 'docker', 'kubernetes', 'x86_64'],
    },
    {
        name: 'cihub-runner-03.eu.rbx.cihub.io',
        owner: 'myorg',
        arch: 'arm64',
        cpu: 4,
        ram: 4096, // 4GB in MB
        status: 'offline',
        created_at: Math.floor(Date.now() / 1000) - 86400 * 15, // 15 days ago
        last_seen_at: Math.floor(Date.now() / 1000) - 86400 * 2, // 2 days ago
        updated_at: Math.floor(Date.now() / 1000) - 86400 * 2,
        labels: ['linux', 'arm64', 'mobile-testing'],
    },
    {
        name: 'cihub-runner-04.us.sjc.cihub.io',
        owner: 'myorg',
        arch: 'x86_64',
        cpu: 2,
        ram: 4096, // 4GB in MB
        status: 'paused',
        created_at: Math.floor(Date.now() / 1000) - 86400 * 10, // 10 days ago
        last_seen_at: Math.floor(Date.now() / 1000) - 3600, // 1 hour ago
        updated_at: Math.floor(Date.now() / 1000) - 7200, // 2 hours ago
        labels: ['linux', 'python', 'node'],
    },
    {
        name: 'cihub-runner-05.us.ewr.cihub.io',
        owner: 'myorg',
        arch: 'x86_64',
        cpu: 16,
        ram: 32768, // 32GB in MB
        status: 'unhealthy',
        created_at: Math.floor(Date.now() / 1000) - 86400 * 5, // 5 days ago
        last_seen_at: Math.floor(Date.now() / 1000) - 1800, // 30 minutes ago
        updated_at: Math.floor(Date.now() / 1000) - 1800,
        labels: ['linux', 'gpu', 'cuda', 'x86_64'],
    },
]

export function useMachines() {
    const { selectedInstallation } = useInstallation()

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'machines'],
        queryFn: async () => {
            if (!selectedInstallation) {
                return []
            }

            try {
                const response = await fetch(`/api/installations/${selectedInstallation.login}/machines`)
                if (!response.ok) {
                    throw new Error('Failed to fetch machines')
                }
                const data = (await response.json()) as PaginatedApiResponse<Machine>
                if (data.error) {
                    throw new Error(data.reason || 'Failed to fetch machines')
                }
                // Return mock data if API returns empty
                return data.data && Array.isArray(data.data) && data.data.length > 0 ? data.data : MOCK_MACHINES
            } catch (error) {
                console.warn('Failed to fetch machines from API, using mock data:', error)
                // Return mock data as fallback
                return MOCK_MACHINES
            }
        },
        enabled: !!selectedInstallation,
        retry: false,
    })
}
