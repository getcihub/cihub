import type { PaginatedApiResponse } from '@/types/api';
import type { RunnerWithJob } from '@/types/runner';
import { useQuery } from '@tanstack/react-query';

import { useInstallation } from './useInstallation';

// Set to true to use mock data, false to use real API
const USE_MOCK_DATA = true;

// Mock data for development
const createMockRunners = (owner: string): RunnerWithJob[] => {
    const now = Math.floor(Date.now() / 1000);
    return [
        {
            id: 1,
            name: 'cihub-runner-abc123',
            machine: 'prod-worker-01',
            installation_id: 1,
            owner,
            status: 'busy',
            arch: 'linux/amd64',
            cpu: 4,
            ram: 8192,
            group_id: 1,
            labels: ['ubuntu-latest', 'self-hosted', 'large'],
            cancelled: 0,
            created: now - 3600,
            accepted: now - 3500,
            started: now - 3400,
            stopped: 0,
            updated: now - 60,
            job: {
                id: 101,
                name: 'build-and-test',
                workflow: 'CI Pipeline',
                repo: `${owner}/web-app`,
                branch: 'main',
                sha: 'a1b2c3d4e5f6',
                author_login: 'johndoe',
                author_avatar:
                    'https://avatars.githubusercontent.com/u/1?v=4',
                started_at: now - 3400,
            },
        },
        {
            id: 2,
            name: 'cihub-runner-def456',
            machine: 'prod-worker-01',
            installation_id: 1,
            owner,
            status: 'busy',
            arch: 'linux/amd64',
            cpu: 2,
            ram: 4096,
            group_id: 1,
            labels: ['ubuntu-latest', 'self-hosted'],
            cancelled: 0,
            created: now - 1800,
            accepted: now - 1700,
            started: now - 1600,
            stopped: 0,
            updated: now - 30,
            job: {
                id: 102,
                name: 'deploy-staging',
                workflow: 'Deploy',
                repo: `${owner}/api-service`,
                branch: 'feature/new-auth',
                sha: 'b2c3d4e5f6a7',
                author_login: 'janedoe',
                author_avatar:
                    'https://avatars.githubusercontent.com/u/2?v=4',
                started_at: now - 1600,
            },
        },
        {
            id: 3,
            name: 'cihub-runner-ghi789',
            machine: 'prod-worker-02',
            installation_id: 1,
            owner,
            status: 'idle',
            arch: 'linux/arm64',
            cpu: 8,
            ram: 16384,
            group_id: 1,
            labels: ['ubuntu-latest', 'self-hosted', 'arm64', 'xlarge'],
            cancelled: 0,
            created: now - 7200,
            accepted: now - 7100,
            started: now - 7000,
            stopped: now - 3600,
            updated: now - 120,
        },
        {
            id: 4,
            name: 'cihub-runner-jkl012',
            machine: 'dev-worker-01',
            installation_id: 1,
            owner,
            status: 'pending',
            arch: 'linux/amd64',
            cpu: 2,
            ram: 4096,
            group_id: 2,
            labels: ['ubuntu-latest', 'self-hosted', 'dev'],
            cancelled: 0,
            created: now - 300,
            accepted: 0,
            started: 0,
            stopped: 0,
            updated: now - 300,
        },
        {
            id: 5,
            name: 'cihub-runner-mno345',
            machine: 'prod-worker-01',
            installation_id: 1,
            owner,
            status: 'idle',
            arch: 'linux/amd64',
            cpu: 4,
            ram: 8192,
            group_id: 1,
            labels: ['ubuntu-latest', 'self-hosted', 'large'],
            cancelled: 0,
            created: now - 10800,
            accepted: now - 10700,
            started: now - 10600,
            stopped: now - 7200,
            updated: now - 7200,
        },
        {
            id: 6,
            name: 'cihub-runner-pqr678',
            machine: 'prod-worker-02',
            installation_id: 1,
            owner,
            status: 'busy',
            arch: 'linux/arm64',
            cpu: 4,
            ram: 8192,
            group_id: 1,
            labels: ['ubuntu-latest', 'self-hosted', 'arm64'],
            cancelled: 0,
            created: now - 900,
            accepted: now - 850,
            started: now - 800,
            stopped: 0,
            updated: now - 15,
            job: {
                id: 103,
                name: 'run-integration-tests',
                workflow: 'Integration Tests',
                repo: `${owner}/mobile-app`,
                branch: 'develop',
                sha: 'c3d4e5f6a7b8',
                author_login: 'devuser',
                author_avatar:
                    'https://avatars.githubusercontent.com/u/3?v=4',
                started_at: now - 800,
            },
        },
    ];
};

async function fetchRunnersFromApi(
    login: string,
): Promise<RunnerWithJob[]> {
    const response = await fetch(`/api/installations/${login}/runners`);
    if (!response.ok) {
        throw new Error('Failed to fetch runners');
    }
    const data = (await response.json()) as PaginatedApiResponse<RunnerWithJob>;
    if (data.error) {
        throw new Error(data.reason || 'Failed to fetch runners');
    }
    return data.data || [];
}

async function fetchRunnerDetailFromApi(
    login: string,
    runnerName: string,
): Promise<RunnerWithJob | null> {
    const response = await fetch(
        `/api/installations/${login}/runners/${runnerName}`,
    );
    if (!response.ok) {
        throw new Error('Failed to fetch runner');
    }
    const data = (await response.json()) as {
        data: RunnerWithJob;
        error?: boolean;
        reason?: string;
    };
    if (data.error) {
        throw new Error(data.reason || 'Failed to fetch runner');
    }
    return data.data || null;
}

export function useRunners() {
    const { selectedInstallation } = useInstallation();

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'runners'],
        queryFn: async () => {
            if (!selectedInstallation) {
                return [];
            }

            if (USE_MOCK_DATA) {
                // Simulate network delay
                await new Promise((resolve) => setTimeout(resolve, 300));
                return createMockRunners(selectedInstallation.login);
            }

            return fetchRunnersFromApi(selectedInstallation.login);
        },
        enabled: !!selectedInstallation,
        refetchInterval: 5000,
    });
}

export function useRunnersByMachine(machineName: string) {
    const { data: runners = [] } = useRunners();
    return runners.filter((r) => r.machine === machineName);
}

export function useRunnerDetail(runnerName: string) {
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

            if (USE_MOCK_DATA) {
                // Simulate network delay
                await new Promise((resolve) => setTimeout(resolve, 200));
                const runners = createMockRunners(selectedInstallation.login);
                return runners.find((r) => r.name === runnerName) || null;
            }

            return fetchRunnerDetailFromApi(
                selectedInstallation.login,
                runnerName,
            );
        },
        enabled: !!selectedInstallation && !!runnerName,
        refetchInterval: 5000,
    });
}
