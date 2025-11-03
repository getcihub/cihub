import { useQuery } from '@tanstack/react-query'
import type { Machine } from '../types/machine'
import type { ApiResponse } from '../types/api'
import { useInstallation } from './useInstallation'

export function useMachineDetail(machineName: string | undefined) {
    const { selectedInstallation } = useInstallation()

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'machines', machineName],
        queryFn: async () => {
            if (!selectedInstallation || !machineName) {
                return null
            }

            try {
                const response = await fetch(
                    `/api/installations/${selectedInstallation.login}/machines/${machineName}`
                )
                if (!response.ok) {
                    throw new Error('Failed to fetch machine details')
                }
                const data = (await response.json()) as ApiResponse<Machine>
                if (data.error) {
                    throw new Error(data.reason || 'Failed to fetch machine details')
                }
                return data.data || null
            } catch (error) {
                console.error('Failed to fetch machine details:', error)
                throw error
            }
        },
        enabled: !!selectedInstallation && !!machineName,
        retry: false,
    })
}
