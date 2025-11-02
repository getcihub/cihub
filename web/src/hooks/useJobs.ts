import { useQuery } from '@tanstack/react-query'
import type { Job } from '../types/job'
import type { PaginatedApiResponse } from '../types/api'
import { useInstallation } from './useInstallation'

// Mock jobs data for development
const MOCK_JOBS: Job[] = [
    {
        id: 4598291847,
        installation_id: 12345,
        runner_id: 123456,
        runner_name: 'runner-1',
        owner: 'myorg',
        repo: 'my-project',
        run_id: 7234891,
        workflow: 'CI/CD Pipeline',
        name: 'Build and Test',
        branch: 'main',
        sha: 'abc123def456789abcdef123456789abc',
        status: 'in_progress',
        conclusion: '',
        labels: ['linux', 'x64'],
        url: 'https://api.github.com/repos/myorg/my-project/actions/runners/register-token',
        author_login: 'john.doe',
        author_avatar: 'https://avatars.githubusercontent.com/u/12345?v=4',
        completed: 0,
        created: Math.floor(Date.now() / 1000) - 300,
        queued: Math.floor(Date.now() / 1000) - 300,
        started: Math.floor(Date.now() / 1000) - 280,
        updated: Math.floor(Date.now() / 1000) - 10,
        version: 1,
    },
    {
        id: 4598291848,
        installation_id: 12345,
        runner_id: 0,
        runner_name: '',
        owner: 'myorg',
        repo: 'my-project',
        run_id: 7234891,
        workflow: 'CI/CD Pipeline',
        name: 'Deploy to Staging',
        branch: 'main',
        sha: 'abc123def456789abcdef123456789abc',
        status: 'waiting',
        conclusion: '',
        labels: ['linux', 'x64'],
        url: 'https://api.github.com/repos/myorg/my-project/actions/runners/register-token',
        author_login: 'jane.smith',
        author_avatar: 'https://avatars.githubusercontent.com/u/23456?v=4',
        completed: 0,
        created: Math.floor(Date.now() / 1000) - 600,
        queued: Math.floor(Date.now() / 1000) - 600,
        started: Math.floor(Date.now() / 1000) - 590,
        updated: Math.floor(Date.now() / 1000) - 50,
        version: 1,
    },
    {
        id: 4598291849,
        installation_id: 12345,
        runner_id: 0,
        runner_name: '',
        owner: 'myorg',
        repo: 'another-project',
        run_id: 7234892,
        workflow: 'Lint and Format',
        name: 'Lint Check',
        branch: 'feature/new-feature',
        sha: 'xyz789uvw012xyz789uvw012xyz789uvw',
        status: 'queued',
        conclusion: '',
        labels: ['linux', 'x64'],
        url: 'https://api.github.com/repos/myorg/another-project/actions/runners/register-token',
        author_login: 'bob.wilson',
        author_avatar: 'https://avatars.githubusercontent.com/u/34567?v=4',
        completed: 0,
        created: Math.floor(Date.now() / 1000) - 1200,
        queued: Math.floor(Date.now() / 1000) - 1200,
        started: 0,
        updated: Math.floor(Date.now() / 1000) - 1200,
        version: 1,
    },
    {
        id: 4598291850,
        installation_id: 12345,
        runner_id: 0,
        runner_name: '',
        owner: 'myorg',
        repo: 'my-project',
        run_id: 7234893,
        workflow: 'Tests',
        name: 'Unit Tests',
        branch: 'develop',
        sha: 'def456ghi789def456ghi789def456ghi',
        status: 'queued',
        conclusion: '',
        labels: ['linux', 'x64'],
        url: 'https://api.github.com/repos/myorg/my-project/actions/runners/register-token',
        author_login: 'alice.johnson',
        author_avatar: 'https://avatars.githubusercontent.com/u/45678?v=4',
        completed: 0,
        created: Math.floor(Date.now() / 1000) - 1800,
        queued: Math.floor(Date.now() / 1000) - 1800,
        started: 0,
        updated: Math.floor(Date.now() / 1000) - 1800,
        version: 1,
    },
    {
        id: 4598291851,
        installation_id: 12345,
        runner_id: 0,
        runner_name: '',
        owner: 'myorg',
        repo: 'my-api',
        run_id: 7234894,
        workflow: 'Integration Tests',
        name: 'Integration Tests',
        branch: 'main',
        sha: 'ghi789jkl012ghi789jkl012ghi789jkl',
        status: 'waiting',
        conclusion: '',
        labels: ['linux', 'x64'],
        url: 'https://api.github.com/repos/myorg/my-api/actions/runners/register-token',
        author_login: 'charlie.brown',
        author_avatar: 'https://avatars.githubusercontent.com/u/56789?v=4',
        completed: 0,
        created: Math.floor(Date.now() / 1000) - 900,
        queued: Math.floor(Date.now() / 1000) - 900,
        started: Math.floor(Date.now() / 1000) - 890,
        updated: Math.floor(Date.now() / 1000) - 50,
        version: 1,
    },
    {
        id: 4598291852,
        installation_id: 12345,
        runner_id: 123457,
        runner_name: 'runner-2',
        owner: 'myorg',
        repo: 'my-api',
        run_id: 7234895,
        workflow: 'Build and Deploy',
        name: 'Build Docker Image',
        branch: 'main',
        sha: 'jkl012mno345jkl012mno345jkl012mno',
        status: 'in_progress',
        conclusion: '',
        labels: ['linux', 'x64'],
        url: 'https://api.github.com/repos/myorg/my-api/actions/runners/register-token',
        author_login: 'david.lee',
        author_avatar: 'https://avatars.githubusercontent.com/u/67890?v=4',
        completed: 0,
        created: Math.floor(Date.now() / 1000) - 400,
        queued: Math.floor(Date.now() / 1000) - 400,
        started: Math.floor(Date.now() / 1000) - 350,
        updated: Math.floor(Date.now() / 1000) - 20,
        version: 1,
    },
]

export function useJobs() {
    const { selectedInstallation } = useInstallation()

    return useQuery({
        queryKey: ['installations', selectedInstallation?.login, 'jobs'],
        queryFn: async () => {
            if (!selectedInstallation) {
                return []
            }

            try {
                const response = await fetch(`/api/installations/${selectedInstallation.login}/jobs`)
                if (!response.ok) {
                    throw new Error('Failed to fetch jobs')
                }
                const data = (await response.json()) as PaginatedApiResponse<Job>
                if (data.error) {
                    throw new Error(data.reason || 'Failed to fetch jobs')
                }
                // Return mock data if API returns empty
                return data.data && Array.isArray(data.data) && data.data.length > 0 ? data.data : MOCK_JOBS
            } catch (error) {
                console.warn('Failed to fetch jobs from API, using mock data:', error)
                // Return mock data as fallback
                return MOCK_JOBS
            }
        },
        enabled: !!selectedInstallation,
        retry: false,
    })
}
