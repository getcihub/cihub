import { useQuery } from '@tanstack/react-query'
import type { Runner } from '@/types/runner'
import type { PaginatedApiResponse } from '@/types/api'
import { useInstallation } from './useInstallation'

export function useRunners() {
    const { selectedInstallation } = useInstallation()

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'runners'],
        queryFn: async () => {
            if (!selectedInstallation) {
                return []
            }

            const response = await fetch(`/api/installations/${selectedInstallation.login}/runners`)
            if (!response.ok) {
                throw new Error('Failed to fetch runners')
            }
            const data = (await response.json()) as PaginatedApiResponse<Runner>
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch runners')
            }
            return data.data || []
        },
        enabled: !!selectedInstallation,
        refetchInterval: 5000,
    })
}

export function useRunnersByMachine(machineName: string) {
    const { data: runners = [] } = useRunners()
    return runners.filter((r) => r.machine === machineName)
}
