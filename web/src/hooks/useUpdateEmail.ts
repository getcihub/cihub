import { useMutation, useQueryClient } from '@tanstack/react-query'
import type { ApiResponse, User } from '../types/user'

interface UpdateEmailPayload {
    email: string
}

export function useUpdateEmail() {
    const queryClient = useQueryClient()

    return useMutation({
        mutationFn: async (payload: UpdateEmailPayload) => {
            const response = await fetch('/api/user', {
                method: 'PATCH',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(payload),
            })

            if (!response.ok) {
                throw new Error('Failed to update email')
            }

            const data: ApiResponse<User> = await response.json()
            if (data.error) {
                throw new Error(data.reason || 'Failed to update email')
            }

            return data.data
        },
        onSuccess: () => {
            // Invalidate user query to refetch
            queryClient.invalidateQueries({ queryKey: ['user'] })
        },
    })
}
