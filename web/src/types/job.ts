export const JobStatusQueued = 'queued';
export const JobStatusWaiting = 'waiting';
export const JobStatusInProgress = 'in_progress';
export const JobStatusCompleted = 'completed';

export type JobStatus =
    | typeof JobStatusQueued
    | typeof JobStatusWaiting
    | typeof JobStatusInProgress
    | typeof JobStatusCompleted;

export interface Job {
    id: number;
    installation_id: number;
    runner_id: number;
    runner_name: string;
    owner: string;
    repo: string;
    run_id: number;
    workflow: string;
    name: string;
    branch: string;
    sha: string;
    status: JobStatus;
    conclusion: string;
    labels: string[];
    url: string;
    author_login: string;
    author_avatar: string;
    completed: number;
    created: number;
    queued: number;
    started: number;
    updated: number;
    version: number;
}
