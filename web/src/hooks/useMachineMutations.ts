import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useInstallation } from './useInstallation'

export function useMachineMutations() {
    const queryClient = useQueryClient()
    const { selectedInstallation } = useInstallation()

    const pauseMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected')
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ status: 'paused' }),
                }
            )

            if (!response.ok) {
                const error = await response.json().catch(() => ({}))
                throw new Error(error.reason || 'Failed to pause machine')
            }

            return response.json()
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: ['installations', selectedInstallation?.login, 'machines'],
            })
        },
    })

    const resumeMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected')
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ status: 'online' }),
                }
            )

            if (!response.ok) {
                const error = await response.json().catch(() => ({}))
                throw new Error(error.reason || 'Failed to resume machine')
            }

            return response.json()
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: ['installations', selectedInstallation?.login, 'machines'],
            })
        },
    })

    const restartMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected')
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ action: 'restart' }),
                }
            )

            if (!response.ok) {
                const error = await response.json().catch(() => ({}))
                throw new Error(error.reason || 'Failed to restart machine')
            }

            return response.json()
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: ['installations', selectedInstallation?.login, 'machines'],
            })
        },
    })

    const deleteMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected')
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'DELETE',
                }
            )

            if (!response.ok) {
                const error = await response.json().catch(() => ({}))
                throw new Error(error.reason || 'Failed to delete machine')
            }

            return response.json()
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: ['installations', selectedInstallation?.login, 'machines'],
            })
        },
    })

    return {
        pauseMachine,
        resumeMachine,
        restartMachine,
        deleteMachine,
    }
}
