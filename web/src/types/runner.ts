export const RunnerStatusPending = 'pending';
export const RunnerStatusRegistered = 'registered';
export const RunnerStatusIdle = 'idle';
export const RunnerStatusBusy = 'busy';
export const RunnerStatusCompleted = 'completed';

export type RunnerStatus =
    | typeof RunnerStatusPending
    | typeof RunnerStatusRegistered
    | typeof RunnerStatusIdle
    | typeof RunnerStatusBusy
    | typeof RunnerStatusCompleted;

export interface Runner {
    name: string;
    machine: string;
    id: number;
    installation_id: number;
    owner: string;
    status: RunnerStatus;
    arch: string;
    cpu: number;
    ram: number;
    group_id: number;
    labels: string[];
    cancelled: number;
    created: number;
    accepted: number;
    started: number;
    stopped: number;
    updated: number;
}

// Extended runner with job information for the UI
export interface RunnerWithJob extends Runner {
    job?: {
        id: number;
        name: string;
        workflow: string;
        repo: string;
        branch: string;
        sha: string;
        author_login: string;
        author_avatar: string;
        started_at: number;
    };
}
