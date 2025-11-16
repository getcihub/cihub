import type { PaginatedApiResponse } from '@/types/api';
import type { Machine } from '@/types/machine';
import { useQuery } from '@tanstack/react-query';

import { useInstallation } from './useInstallation';

export function useMachines() {
    const { selectedInstallation } = useInstallation();

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'machines'],
        queryFn: async () => {
            if (!selectedInstallation) {
                return [];
            }

            const response = await fetch(
                `/api/installations/${selectedInstallation.login}/machines`,
            );
            if (!response.ok) {
                throw new Error('Failed to fetch machines');
            }
            const data =
                (await response.json()) as PaginatedApiResponse<Machine>;
            if (data.error) {
                throw new Error(data.reason || 'Failed to fetch machines');
            }
            return data.data || [];
        },
        enabled: !!selectedInstallation,
        refetchInterval: 5000,
    });
}
