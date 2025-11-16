import { Button } from '@/components/Button';
import { Card } from '@/components/Card';
import { JobStatusBadge } from '@/components/JobStatusBadge';
import { Skeleton } from '@/components/Skeleton';
import { useInstallation } from '@/hooks/useInstallation';
import { useJobDetail } from '@/hooks/useJobDetail';
import { useRunnerDetail } from '@/hooks/useRunnerDetail';
import {
    RiArrowLeftLine,
    RiArrowRightLine,
    RiBriefcaseLine,
    RiCheckLine,
    RiCpuLine,
    RiExternalLinkLine,
    RiRam2Line,
    RiServerLine,
    RiTimeLine,
} from '@remixicon/react';
import { useNavigate, useParams, useSearch } from '@tanstack/react-router';

export function JobDetailPage() {
    const { jobId } = useParams({ from: '/$login/jobs/$jobId' });
    const navigate = useNavigate({ from: '/$login/jobs/$jobId' });
    const searchParams = useSearch({ from: '/$login/jobs/$jobId' }) as
        | { tab?: string }
        | undefined;
    const { selectedInstallation } = useInstallation();
    const { data: job, isLoading, error } = useJobDetail(jobId);
    const { data: assignedRunner } = useRunnerDetail(job?.runner_name);

    const handleBack = () => {
        if (selectedInstallation) {
            navigate({
                to: '/$login/jobs',
                params: { login: selectedInstallation.login },
                search: searchParams?.tab
                    ? { tab: searchParams.tab }
                    : undefined,
            });
        }
    };

    const handleMachineClick = () => {
        if (selectedInstallation && assignedRunner) {
            navigate({
                to: '/$login/machines/$name',
                params: {
                    login: selectedInstallation.login,
                    name: assignedRunner.machine,
                },
            });
        }
    };

    // Helper to format duration between timestamps
    const formatDuration = (start: number, end: number) => {
        if (start <= 0 || end <= 0) return '-';
        const seconds = Math.max(0, end - start);
        const minutes = Math.floor(seconds / 60);
        const secs = seconds % 60;
        if (minutes === 0) return `${secs}s`;
        return `${minutes}m ${secs}s`;
    };

    // Calculate queue wait time (created -> started)
    const queueWaitTime =
        job && job.created > 0 && job.started > 0
            ? job.started - job.created
            : 0;

    // Calculate total time
    const totalTime =
        job && job.created > 0 && job.completed > 0
            ? job.completed - job.created
            : 0;

    if (isLoading) {
        return (
            <div className="space-y-6">
                <div className="flex items-center gap-4">
                    <Button
                        onClick={handleBack}
                        variant="ghost"
                        className="gap-2"
                    >
                        <RiArrowLeftLine className="size-4" />
                        Back
                    </Button>
                </div>
                <Skeleton className="h-12 w-full rounded-lg" />
                <div className="space-y-4">
                    {[...Array(3)].map((_, i) => (
                        <Skeleton key={i} className="h-32 w-full rounded-lg" />
                    ))}
                </div>
            </div>
        );
    }

    if (error || !job) {
        return (
            <div className="space-y-6">
                <div className="flex items-center gap-4">
                    <Button
                        onClick={handleBack}
                        variant="ghost"
                        className="gap-2"
                    >
                        <RiArrowLeftLine className="size-4" />
                        Back
                    </Button>
                </div>
                <Card className="p-8 text-center">
                    <RiBriefcaseLine
                        className="size-12 text-gray-300 mx-auto mb-4"
                        aria-hidden="true"
                    />
                    <p className="text-lg font-medium text-gray-900">
                        Job not found
                    </p>
                    {error && (
                        <p className="text-sm text-gray-600 mt-2">
                            {error.message}
                        </p>
                    )}
                </Card>
            </div>
        );
    }

    return (
        <div className="space-y-6">
            <div className="flex items-center gap-4">
                <Button onClick={handleBack} variant="ghost" className="gap-2">
                    <RiArrowLeftLine className="size-4" />
                    Back to Jobs
                </Button>
            </div>

            {/* Header */}
            <div className="flex items-start justify-between gap-4 mb-6">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900 mb-2">
                        {job.name}
                    </h1>
                    <div className="flex items-center gap-3 mt-2">
                        <JobStatusBadge status={job.status} size="md" />
                        {job.conclusion && (
                            <span className="text-sm text-gray-600">
                                Conclusion:{' '}
                                <span className="font-medium capitalize">
                                    {job.conclusion}
                                </span>
                            </span>
                        )}
                    </div>
                </div>
                {job.url && (
                    <Button
                        onClick={() => window.open(job.url, '_blank')}
                        variant="secondary"
                        className="gap-2 flex-shrink-0"
                    >
                        <RiExternalLinkLine className="size-4" />
                        View on GitHub
                    </Button>
                )}
            </div>

            {/* Duration Summary */}
            {job.completed > 0 && (
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
                    <Card className="p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="text-sm font-medium text-gray-600">
                                    Queue Wait Time
                                </p>
                                <div className="mt-2">
                                    <p className="text-2xl font-bold text-gray-900">
                                        {formatDuration(
                                            job.created,
                                            job.started,
                                        )}
                                    </p>
                                    <p className="text-xs text-gray-500 mt-1">
                                        Time before execution
                                    </p>
                                </div>
                            </div>
                            <div className="rounded-lg bg-yellow-100 p-3">
                                <RiTimeLine
                                    className="size-6 text-yellow-600"
                                    aria-hidden="true"
                                />
                            </div>
                        </div>
                    </Card>

                    <Card className="p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="text-sm font-medium text-gray-600">
                                    Execution Time
                                </p>
                                <div className="mt-2">
                                    <p className="text-2xl font-bold text-gray-900">
                                        {formatDuration(
                                            job.started,
                                            job.completed,
                                        )}
                                    </p>
                                    <p className="text-xs text-gray-500 mt-1">
                                        Actual job duration
                                    </p>
                                </div>
                            </div>
                            <div className="rounded-lg bg-purple-100 p-3">
                                <RiCheckLine
                                    className="size-6 text-purple-600"
                                    aria-hidden="true"
                                />
                            </div>
                        </div>
                    </Card>

                    <Card className="p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="text-sm font-medium text-gray-600">
                                    Total Time
                                </p>
                                <div className="mt-2">
                                    <p className="text-2xl font-bold text-gray-900">
                                        {formatDuration(
                                            job.created,
                                            job.completed,
                                        )}
                                    </p>
                                    <p className="text-xs text-gray-500 mt-1">
                                        {queueWaitTime > 0
                                            ? Math.round(
                                                  (queueWaitTime / totalTime) *
                                                      100,
                                              )
                                            : 0}
                                        % queue wait
                                    </p>
                                </div>
                            </div>
                            <div className="rounded-lg bg-blue-100 p-3">
                                <RiTimeLine
                                    className="size-6 text-blue-600"
                                    aria-hidden="true"
                                />
                            </div>
                        </div>
                    </Card>
                </div>
            )}

            {/* Job Information Card */}
            <Card className="p-6">
                <h2 className="text-sm font-semibold text-gray-900 mb-4">
                    Job Information
                </h2>

                {/* Top Row: Author, Workflow, Repository */}
                <div className="flex flex-wrap items-center gap-8 pb-6 border-b border-gray-200">
                    {/* Author */}
                    <div className="flex items-center gap-3">
                        <img
                            src={job.author_avatar}
                            alt={job.author_login}
                            className="size-6 rounded-full object-cover"
                        />
                        <div>
                            <p className="text-xs font-medium text-gray-600">
                                Triggered by
                            </p>
                            <p className="text-sm font-medium text-gray-900">
                                @{job.author_login}
                            </p>
                        </div>
                    </div>

                    {/* Workflow */}
                    <div className="min-w-0">
                        <p className="text-xs font-medium text-gray-600">
                            Workflow
                        </p>
                        <p className="text-sm font-medium text-gray-900 mt-1 truncate">
                            {job.workflow}
                        </p>
                    </div>

                    {/* Repository */}
                    <div className="min-w-0">
                        <p className="text-xs font-medium text-gray-600">
                            Repository
                        </p>
                        <p className="text-sm font-medium text-gray-900 mt-1">
                            {job.owner}/{job.repo}
                        </p>
                    </div>
                </div>

                {/* Bottom Row: Branch, Commit, Run ID, Labels */}
                <div className="grid grid-cols-2 md:grid-cols-4 gap-6 pt-6">
                    <div>
                        <p className="text-xs font-medium text-gray-600">
                            Branch
                        </p>
                        <p className="text-sm font-medium text-gray-900 mt-1">
                            {job.branch}
                        </p>
                    </div>
                    <div>
                        <p className="text-xs font-medium text-gray-600">
                            Commit
                        </p>
                        <p className="text-sm font-mono text-gray-900 mt-1">
                            {job.sha.substring(0, 7)}
                        </p>
                    </div>
                    <div>
                        <p className="text-xs font-medium text-gray-600">
                            Run ID
                        </p>
                        <p className="text-sm font-mono text-gray-900 mt-1">
                            {job.run_id}
                        </p>
                    </div>
                    {job.labels && job.labels.length > 0 && (
                        <div className="col-span-2 md:col-span-1">
                            <p className="text-xs font-medium text-gray-600 mb-2">
                                Required Labels
                            </p>
                            <div className="flex flex-wrap gap-1">
                                {job.labels.map((label) => (
                                    <span
                                        key={label}
                                        className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-50 text-blue-700 border border-blue-200"
                                    >
                                        {label}
                                    </span>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
            </Card>

            {/* Timeline Card */}
            <Card className="p-6">
                <h2 className="text-sm font-semibold text-gray-900 mb-4">
                    Job Timeline
                </h2>
                <div className="space-y-3">
                    {/* Created Step */}
                    <div className="flex gap-3">
                        <div className="flex flex-col items-center flex-shrink-0">
                            <div className="w-6 h-6 rounded-full bg-blue-100 flex items-center justify-center flex-shrink-0">
                                <RiCheckLine className="size-3 text-blue-600" />
                            </div>
                            {(job.queued > 0 ||
                                job.started > 0 ||
                                job.completed > 0) && (
                                <div className="w-0.5 h-8 bg-gray-300 my-1" />
                            )}
                        </div>
                        <div className="flex-1 pt-0.5 min-w-0">
                            <p className="text-xs font-semibold text-gray-900">
                                Created
                            </p>
                            <p className="text-xs text-gray-500 mt-0.5">
                                {new Date(job.created * 1000).toLocaleString(
                                    'en-US',
                                    {
                                        month: 'short',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    },
                                )}
                            </p>
                        </div>
                    </div>

                    {/* Queued Step */}
                    {job.queued > 0 && (
                        <div className="flex gap-3">
                            <div className="flex flex-col items-center flex-shrink-0">
                                <div className="w-6 h-6 rounded-full bg-blue-100 flex items-center justify-center flex-shrink-0">
                                    <RiCheckLine className="size-3 text-blue-600" />
                                </div>
                                {(job.started > 0 || job.completed > 0) && (
                                    <div className="w-0.5 h-8 bg-gray-300 my-1" />
                                )}
                            </div>
                            <div className="flex-1 pt-0.5 min-w-0">
                                <div className="flex items-center justify-between gap-2">
                                    <p className="text-xs font-semibold text-gray-900">
                                        Queued
                                    </p>
                                    <span className="text-xs text-gray-500 flex-shrink-0">
                                        +
                                        {formatDuration(
                                            job.created,
                                            job.queued,
                                        )}
                                    </span>
                                </div>
                                <p className="text-xs text-gray-500 mt-0.5">
                                    {new Date(job.queued * 1000).toLocaleString(
                                        'en-US',
                                        {
                                            month: 'short',
                                            day: 'numeric',
                                            hour: '2-digit',
                                            minute: '2-digit',
                                        },
                                    )}
                                </p>
                            </div>
                        </div>
                    )}

                    {/* Started Step */}
                    {job.started > 0 && (
                        <div className="flex gap-3">
                            <div className="flex flex-col items-center flex-shrink-0">
                                <div className="w-6 h-6 rounded-full bg-blue-100 flex items-center justify-center flex-shrink-0">
                                    <RiCheckLine className="size-3 text-blue-600" />
                                </div>
                                {job.completed > 0 && (
                                    <div className="w-0.5 h-8 bg-gray-300 my-1" />
                                )}
                            </div>
                            <div className="flex-1 pt-0.5 min-w-0">
                                <div className="flex items-center justify-between gap-2">
                                    <p className="text-xs font-semibold text-gray-900">
                                        Started
                                    </p>
                                    <span className="text-xs text-gray-500 flex-shrink-0">
                                        +
                                        {formatDuration(
                                            job.queued,
                                            job.started,
                                        )}
                                    </span>
                                </div>
                                <p className="text-xs text-gray-500 mt-0.5">
                                    {new Date(
                                        job.started * 1000,
                                    ).toLocaleString('en-US', {
                                        month: 'short',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    })}
                                </p>
                            </div>
                        </div>
                    )}

                    {/* Completed Step */}
                    {job.completed > 0 && (
                        <div className="flex gap-3">
                            <div className="flex flex-col items-center flex-shrink-0">
                                <div className="w-6 h-6 rounded-full bg-green-100 flex items-center justify-center flex-shrink-0">
                                    <RiCheckLine className="size-3 text-green-600" />
                                </div>
                            </div>
                            <div className="flex-1 pt-0.5 min-w-0">
                                <div className="flex items-center justify-between gap-2">
                                    <p className="text-xs font-semibold text-gray-900">
                                        Completed
                                    </p>
                                    <span className="text-xs text-gray-500 flex-shrink-0">
                                        +
                                        {formatDuration(
                                            job.started,
                                            job.completed,
                                        )}
                                    </span>
                                </div>
                                <p className="text-xs text-gray-500 mt-0.5">
                                    {new Date(
                                        job.completed * 1000,
                                    ).toLocaleString('en-US', {
                                        month: 'short',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    })}
                                </p>
                            </div>
                        </div>
                    )}
                </div>
            </Card>

            {/* Runner Assignment */}
            {job?.runner_name && assignedRunner ? (
                <Card className="p-6">
                    <div className="flex items-start gap-4">
                        <div className="rounded-lg bg-blue-100 p-3 flex-shrink-0">
                            <RiServerLine
                                className="size-6 text-blue-600"
                                aria-hidden="true"
                            />
                        </div>
                        <div className="flex-1 min-w-0">
                            <h3 className="text-sm font-semibold text-gray-900 mb-4">
                                Runner Assignment
                            </h3>
                            <div className="space-y-3">
                                <div className="pb-3 border-b border-gray-200">
                                    <p className="text-xs font-medium text-gray-600">
                                        Runner
                                    </p>
                                    <div className="flex items-center gap-2 mt-1">
                                        <p className="text-sm font-medium text-gray-900">
                                            {assignedRunner.name}
                                        </p>
                                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-50 text-blue-700 border border-blue-200 capitalize">
                                            {assignedRunner.status}
                                        </span>
                                    </div>
                                </div>
                                <div className="pb-3 border-b border-gray-200">
                                    <p className="text-xs font-medium text-gray-600">
                                        Machine
                                    </p>
                                    <button
                                        onClick={handleMachineClick}
                                        className="text-xs text-blue-600 hover:text-blue-700 font-medium flex items-center gap-1 mt-1"
                                    >
                                        <span>{assignedRunner.machine}</span>
                                        <RiArrowRightLine className="size-3" />
                                    </button>
                                </div>
                                <div>
                                    <p className="text-xs font-medium text-gray-600 mb-3">
                                        Architecture & Resources
                                    </p>
                                    <div className="flex flex-wrap gap-4">
                                        <div className="flex items-center gap-2">
                                            <span className="text-gray-400">
                                                {assignedRunner.arch ===
                                                'x86_64'
                                                    ? 'x86'
                                                    : assignedRunner.arch ===
                                                        'arm64'
                                                      ? 'ARM'
                                                      : assignedRunner.arch}
                                            </span>
                                            <p className="text-sm font-mono text-gray-900">
                                                {assignedRunner.arch}
                                            </p>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <RiCpuLine
                                                className="size-4 text-blue-600"
                                                aria-hidden="true"
                                            />
                                            <p className="text-sm text-gray-900">
                                                <span className="font-semibold">
                                                    {assignedRunner.cpu}
                                                </span>{' '}
                                                vCPU
                                            </p>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <RiRam2Line
                                                className="size-4 text-blue-600"
                                                aria-hidden="true"
                                            />
                                            <p className="text-sm text-gray-900">
                                                <span className="font-semibold">
                                                    {Math.round(
                                                        assignedRunner.ram /
                                                            1024,
                                                    )}
                                                </span>{' '}
                                                GB RAM
                                            </p>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </Card>
            ) : job.runner_name ? (
                <Card className="p-6">
                    <div className="flex items-start gap-4">
                        <div className="rounded-lg bg-blue-100 p-3 flex-shrink-0">
                            <RiServerLine
                                className="size-6 text-blue-600"
                                aria-hidden="true"
                            />
                        </div>
                        <div className="flex-1">
                            <h3 className="text-sm font-semibold text-gray-900">
                                Runner
                            </h3>
                            <p className="text-sm text-gray-900 mt-2">
                                {job.runner_name}
                            </p>
                        </div>
                    </div>
                </Card>
            ) : null}
        </div>
    );
}
