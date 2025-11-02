import { useQuery } from '@tanstack/react-query'
import type { Installation, Membership } from '../types/installation'
import type { ApiResponse } from '../types/api'

interface InstallationDetailsData {
    installation: Installation
    membership: Membership
}

interface ApiError extends Error {
    status?: number
    reason?: string
}

export function useInstallationDetails(login: string | null) {
    return useQuery({
        queryKey: ['installation', login],
        queryFn: async () => {
            if (!login) {
                throw new Error('Installation login is required')
            }
            const response = await fetch(`/api/installations/${login}`)
            const data = (await response.json()) as ApiResponse<InstallationDetailsData>

            if (!response.ok || data.error) {
                const error: ApiError = new Error(data.reason || 'Failed to fetch installation details')
                error.status = response.status
                error.reason = data.reason
                throw error
            }

            return data.data
        },
        enabled: !!login,
        retry: false,
    })
}
