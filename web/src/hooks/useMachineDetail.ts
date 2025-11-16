import type { ApiResponse } from '@/types/api';
import type { Machine } from '@/types/machine';
import type { Runner } from '@/types/runner';
import { useQuery } from '@tanstack/react-query';

import { useInstallation } from './useInstallation';

export function useMachineDetail(machineName: string | undefined) {
    const { selectedInstallation } = useInstallation();

    return useQuery({
        queryKey: [
            'installations',
            selectedInstallation?.login,
            'machines',
            machineName,
        ],
        queryFn: async () => {
            if (!selectedInstallation || !machineName) {
                return null;
            }

            try {
                // Fetch machine details
                const machineResponse = await fetch(
                    `/api/installations/${selectedInstallation.login}/machines/${machineName}`,
                );
                if (!machineResponse.ok) {
                    throw new Error('Failed to fetch machine details');
                }
                const machineData =
                    (await machineResponse.json()) as ApiResponse<Machine>;
                if (machineData.error) {
                    throw new Error(
                        machineData.reason || 'Failed to fetch machine details',
                    );
                }
                const machine = machineData.data || null;

                // Fetch runners for this machine
                if (machine) {
                    try {
                        const runnersResponse = await fetch(
                            `/api/installations/${selectedInstallation.login}/machines/${machineName}/runners`,
                        );
                        if (runnersResponse.ok) {
                            const runnersData =
                                (await runnersResponse.json()) as ApiResponse<
                                    Runner[]
                                >;
                            if (!runnersData.error) {
                                machine.runners = runnersData.data || [];
                            }
                        }
                    } catch (error) {
                        console.error(
                            'Failed to fetch machine runners:',
                            error,
                        );
                        // Continue without runners if fetch fails
                        machine.runners = [];
                    }
                }

                return machine;
            } catch (error) {
                console.error('Failed to fetch machine details:', error);
                throw error;
            }
        },
        enabled: !!selectedInstallation && !!machineName,
        retry: false,
    });
}
