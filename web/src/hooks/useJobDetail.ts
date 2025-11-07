import { useQuery } from '@tanstack/react-query'
import type { Job } from '@/types/job'
import type { ApiResponse } from '@/types/api'
import { useInstallation } from './useInstallation'

export function useJobDetail(jobId: string | undefined) {
    const { selectedInstallation } = useInstallation()

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'jobs', jobId],
        queryFn: async () => {
            if (!selectedInstallation || !jobId) {
                return null
            }

            try {
                const response = await fetch(
                    `/api/installations/${selectedInstallation.login}/jobs/${jobId}`
                )
                if (!response.ok) {
                    throw new Error('Failed to fetch job details')
                }
                const data = (await response.json()) as ApiResponse<Job>
                if (data.error) {
                    throw new Error(data.reason || 'Failed to fetch job details')
                }
                return data.data || null
            } catch (error) {
                console.error('Failed to fetch job details:', error)
                throw error
            }
        },
        enabled: !!selectedInstallation && !!jobId,
        retry: false,
    })
}
