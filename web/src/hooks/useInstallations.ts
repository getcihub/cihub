import { useQuery } from '@tanstack/react-query'
import type { Installation } from '../types/installation'
import type { PaginatedApiResponse } from '../types/api'

export function useInstallations() {
    return useQuery({
        queryKey: ['user', 'installations'],
        queryFn: async () => {
            const response = await fetch('/api/user/installations')
            if (!response.ok) {
                throw new Error('Failed to fetch installations')
            }
            const data = (await response.json()) as PaginatedApiResponse<Installation>
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch installations')
            }
            return data.data
        },
        retry: false,
    })
}
