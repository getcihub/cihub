import { useQuery } from '@tanstack/react-query'
import type { Job } from '../types/job'
import type { PaginatedApiResponse } from '../types/api'
import { useInstallation } from './useInstallation'

export interface UseJobsOptions {
    status?: 'incomplete' | 'completed'
    limit?: number
    jobId?: number
}

export function useJobs(options?: UseJobsOptions) {
    const { selectedInstallation } = useInstallation()
    const { status = 'incomplete', limit = 25, jobId = 0 } = options || {}

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'jobs', { status, limit, jobId }],
        queryFn: async () => {
            if (!selectedInstallation) {
                return { data: [], hasMore: false }
            }

            const params = new URLSearchParams()
            params.set('status', status)
            params.set('limit', String(limit))
            if (jobId > 0) {
                params.set('job_id', String(jobId))
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/jobs?${params.toString()}`
            )
            if (!response.ok) {
                throw new Error('Failed to fetch jobs')
            }
            const data = (await response.json()) as PaginatedApiResponse<Job>
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch jobs')
            }
            return {
                data: data.data || [],
                hasMore: data.has_more || false,
            }
        },
        enabled: !!selectedInstallation,
        refetchInterval: status === 'incomplete' ? 5000 : undefined,
    })
}
