import { useQuery } from '@tanstack/react-query'
import type { User } from '../types/user'
import type { ApiResponse } from '../types/api'

export function useUser() {
    return useQuery({
        queryKey: ['user'],
        queryFn: async () => {
            const response = await fetch('/api/user')
            if (!response.ok) {
                throw new Error('Not authenticated')
            }
            const data: ApiResponse<User> = await response.json()
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch user')
            }
            return data.data
        },
        retry: false,
    })
}
