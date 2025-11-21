import { useMutation, useQueryClient } from '@tanstack/react-query';

import { useInstallation } from './useInstallation';

export function useMachineMutations() {
    const queryClient = useQueryClient();
    const { selectedInstallation } = useInstallation();

    const pauseMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected');
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ status: 'paused' }),
                },
            );

            if (!response.ok) {
                const error = await response.json().catch(() => ({}));
                throw new Error(error.reason || 'Failed to pause machine');
            }

            return response.json();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: [
                    'installations',
                    selectedInstallation?.login,
                    'machines',
                ],
            });
        },
    });

    const resumeMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected');
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ status: 'online' }),
                },
            );

            if (!response.ok) {
                const error = await response.json().catch(() => ({}));
                throw new Error(error.reason || 'Failed to resume machine');
            }

            return response.json();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: [
                    'installations',
                    selectedInstallation?.login,
                    'machines',
                ],
            });
        },
    });

    const restartMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected');
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({ action: 'restart' }),
                },
            );

            if (!response.ok) {
                const error = await response.json().catch(() => ({}));
                throw new Error(error.reason || 'Failed to restart machine');
            }

            return response.json();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: [
                    'installations',
                    selectedInstallation?.login,
                    'machines',
                ],
            });
        },
    });

    const deleteMachine = useMutation({
        mutationFn: async (machineName: string) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected');
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                {
                    method: 'DELETE',
                },
            );

            if (!response.ok) {
                const error = await response.json().catch(() => ({}));
                throw new Error(error.reason || 'Failed to delete machine');
            }

            return response.json();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: [
                    'installations',
                    selectedInstallation?.login,
                    'machines',
                ],
            });
        },
    });

    const createMachine = useMutation({
        mutationFn: async (machineData: {
            name: string;
            arch: string;
            cpu: number;
            ram: number;
            labels: string[];
        }) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected');
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines`,
                {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify(machineData),
                },
            );

            if (!response.ok) {
                const error = await response.json().catch(() => ({}));
                throw new Error(error.reason || 'Failed to create machine');
            }

            return response.json();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: [
                    'installations',
                    selectedInstallation?.login,
                    'machines',
                ],
            });
        },
    });

    const updateMachineLimit = useMutation({
        mutationFn: async (data: {
            machineName: string;
            cpu: number;
            ram: number;
        }) => {
            if (!selectedInstallation) {
                throw new Error('No installation selected');
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines/${data.machineName}`,
                {
                    method: 'PATCH',
                    headers: {
                        'Content-Type': 'application/json',
                    },
                    body: JSON.stringify({
                        limit: {
                            cpu: data.cpu,
                            ram: data.ram,
                        },
                    }),
                },
            );

            if (!response.ok) {
                const error = await response.json().catch(() => ({}));
                throw new Error(
                    error.reason || 'Failed to update machine limits',
                );
            }

            return response.json();
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: [
                    'installations',
                    selectedInstallation?.login,
                    'machines',
                ],
            });
        },
    });

    return {
        createMachine,
        pauseMachine,
        resumeMachine,
        restartMachine,
        deleteMachine,
        updateMachineLimit,
    };
}
