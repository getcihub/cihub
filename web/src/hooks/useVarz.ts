import { useQuery } from '@tanstack/react-query'
import type { ApiResponse } from '@/types/api'

interface VarzData {
    github: {
        name: string
    }
}

export function useVarz() {
    return useQuery({
        queryKey: ['varz'],
        queryFn: async () => {
            const response = await fetch('/api/varz')
            if (!response.ok) {
                throw new Error('Failed to fetch varz')
            }
            const data: ApiResponse<VarzData> = await response.json()
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch varz')
            }
            return data.data
        },
        staleTime: 1000 * 60 * 5, // 5 minutes
    })
}
