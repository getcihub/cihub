import { useQuery } from '@tanstack/react-query'
import type { UserEmail } from '@/types/user'
import type { ApiResponse } from '@/types/api'

export function useEmails() {
    return useQuery({
        queryKey: ['emails'],
        queryFn: async () => {
            const response = await fetch('/api/user/emails')
            if (!response.ok) {
                throw new Error('Failed to fetch emails')
            }
            const data: ApiResponse<UserEmail[]> = await response.json()
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch emails')
            }
            return data.data
        },
    })
}
