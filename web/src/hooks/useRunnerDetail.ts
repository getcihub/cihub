import type { ApiResponse } from '@/types/api';
import type { Runner } from '@/types/runner';
import { useQuery } from '@tanstack/react-query';

import { useInstallation } from './useInstallation';

export function useRunnerDetail(runnerName: string | undefined) {
    const { selectedInstallation } = useInstallation();

    return useQuery({
        queryKey: [
            'installations',
            selectedInstallation?.login,
            'runners',
            runnerName,
        ],
        queryFn: async () => {
            if (!selectedInstallation || !runnerName) {
                return null;
            }

            try {
                const response = await fetch(
                    `/api/installations/${selectedInstallation.login}/runners/${runnerName}`,
                );
                if (!response.ok) {
                    throw new Error('Failed to fetch runner details');
                }
                const data = (await response.json()) as ApiResponse<Runner>;
                if (data.error) {
                    throw new Error(
                        data.reason || 'Failed to fetch runner details',
                    );
                }
                return data.data || null;
            } catch (error) {
                console.error('Failed to fetch runner details:', error);
                throw error;
            }
        },
        enabled: !!selectedInstallation && !!runnerName,
        retry: false,
    });
}
