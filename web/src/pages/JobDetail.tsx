import { JobStatusBadge } from '@/components/JobStatusBadge';
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
import { motion } from 'framer-motion';

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
                    <button
                        onClick={handleBack}
                        className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                    >
                        <RiArrowLeftLine className="size-4" />
                        Back
                    </button>
                </div>
                <div className="h-12 animate-pulse rounded-xl border border-white/10 bg-white/[0.02]" />
                <div className="space-y-4">
                    {[...Array(3)].map((_, i) => (
                        <div
                            key={i}
                            className="h-32 animate-pulse rounded-xl border border-white/10 bg-white/[0.02]"
                        />
                    ))}
                </div>
            </div>
        );
    }

    if (error || !job) {
        return (
            <div className="space-y-6">
                <div className="flex items-center gap-4">
                    <button
                        onClick={handleBack}
                        className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                    >
                        <RiArrowLeftLine className="size-4" />
                        Back
                    </button>
                </div>
                <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center">
                    <RiBriefcaseLine
                        className="mx-auto mb-4 size-12 text-white/20"
                        aria-hidden="true"
                    />
                    <p className="font-display text-lg text-white">
                        Job not found
                    </p>
                    {error && (
                        <p className="mt-2 font-mono text-sm text-white/50">
                            {error.message}
                        </p>
                    )}
                </div>
            </div>
        );
    }

    return (
        <div className="space-y-6">
            <motion.div
                className="flex items-center gap-4"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
            >
                <button
                    onClick={handleBack}
                    className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                >
                    <RiArrowLeftLine className="size-4" />
                    Back to Jobs
                </button>
            </motion.div>

            {/* Header */}
            <motion.div
                className="mb-6 flex items-start justify-between gap-4"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.1 }}
            >
                <div>
                    <h1 className="mb-2 font-display text-3xl text-white">
                        {job.name}
                    </h1>
                    <div className="mt-2 flex items-center gap-3">
                        <JobStatusBadge status={job.status} size="md" />
                        {job.conclusion && (
                            <span className="font-mono text-sm text-white/50">
                                Conclusion:{' '}
                                <span className="capitalize text-white/70">
                                    {job.conclusion}
                                </span>
                            </span>
                        )}
                    </div>
                </div>
                {job.url && (
                    <button
                        onClick={() => window.open(job.url, '_blank')}
                        className="inline-flex flex-shrink-0 items-center gap-2 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                    >
                        <RiExternalLinkLine className="size-4" />
                        View on GitHub
                    </button>
                )}
            </motion.div>

            {/* Duration Summary */}
            {job.completed > 0 && (
                <motion.div
                    className="mb-6 grid grid-cols-1 gap-4 md:grid-cols-3"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.2 }}
                >
                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="font-mono text-sm text-white/50">
                                    Queue Wait Time
                                </p>
                                <div className="mt-2">
                                    <p className="font-display text-2xl text-white">
                                        {formatDuration(job.created, job.started)}
                                    </p>
                                    <p className="mt-1 font-mono text-xs text-white/30">
                                        Time before execution
                                    </p>
                                </div>
                            </div>
                            <div className="rounded-lg bg-yellow-500/20 p-3">
                                <RiTimeLine
                                    className="size-6 text-yellow-400"
                                    aria-hidden="true"
                                />
                            </div>
                        </div>
                    </div>

                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="font-mono text-sm text-white/50">
                                    Execution Time
                                </p>
                                <div className="mt-2">
                                    <p className="font-display text-2xl text-white">
                                        {formatDuration(job.started, job.completed)}
                                    </p>
                                    <p className="mt-1 font-mono text-xs text-white/30">
                                        Actual job duration
                                    </p>
                                </div>
                            </div>
                            <div className="rounded-lg bg-purple-500/20 p-3">
                                <RiCheckLine
                                    className="size-6 text-purple-400"
                                    aria-hidden="true"
                                />
                            </div>
                        </div>
                    </div>

                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="font-mono text-sm text-white/50">
                                    Total Time
                                </p>
                                <div className="mt-2">
                                    <p className="font-display text-2xl text-white">
                                        {formatDuration(job.created, job.completed)}
                                    </p>
                                    <p className="mt-1 font-mono text-xs text-white/30">
                                        {queueWaitTime > 0
                                            ? Math.round(
                                                  (queueWaitTime / totalTime) * 100,
                                              )
                                            : 0}
                                        % queue wait
                                    </p>
                                </div>
                            </div>
                            <div className="rounded-lg bg-blue-500/20 p-3">
                                <RiTimeLine
                                    className="size-6 text-blue-400"
                                    aria-hidden="true"
                                />
                            </div>
                        </div>
                    </div>
                </motion.div>
            )}

            {/* Job Information Card */}
            <motion.div
                className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.3 }}
            >
                <h2 className="mb-4 font-mono text-sm font-medium text-white">
                    Job Information
                </h2>

                {/* Top Row: Author, Workflow, Repository */}
                <div className="flex flex-wrap items-center gap-8 border-b border-white/10 pb-6">
                    {/* Author */}
                    <div className="flex items-center gap-3">
                        <img
                            src={job.author_avatar}
                            alt={job.author_login}
                            className="size-6 rounded-full border border-white/10 object-cover"
                        />
                        <div>
                            <p className="font-mono text-xs text-white/50">
                                Triggered by
                            </p>
                            <p className="font-mono text-sm text-white">
                                @{job.author_login}
                            </p>
                        </div>
                    </div>

                    {/* Workflow */}
                    <div className="min-w-0">
                        <p className="font-mono text-xs text-white/50">
                            Workflow
                        </p>
                        <p className="mt-1 truncate font-mono text-sm text-white">
                            {job.workflow}
                        </p>
                    </div>

                    {/* Repository */}
                    <div className="min-w-0">
                        <p className="font-mono text-xs text-white/50">
                            Repository
                        </p>
                        <p className="mt-1 font-mono text-sm text-white">
                            {job.owner}/{job.repo}
                        </p>
                    </div>
                </div>

                {/* Bottom Row: Branch, Commit, Run ID, Labels */}
                <div className="grid grid-cols-2 gap-6 pt-6 md:grid-cols-4">
                    <div>
                        <p className="font-mono text-xs text-white/50">Branch</p>
                        <p className="mt-1 font-mono text-sm text-white">
                            {job.branch}
                        </p>
                    </div>
                    <div>
                        <p className="font-mono text-xs text-white/50">Commit</p>
                        <p className="mt-1 font-mono text-sm text-white">
                            {job.sha.substring(0, 7)}
                        </p>
                    </div>
                    <div>
                        <p className="font-mono text-xs text-white/50">Run ID</p>
                        <p className="mt-1 font-mono text-sm text-white">
                            {job.run_id}
                        </p>
                    </div>
                    {job.labels && job.labels.length > 0 && (
                        <div className="col-span-2 md:col-span-1">
                            <p className="mb-2 font-mono text-xs text-white/50">
                                Required Labels
                            </p>
                            <div className="flex flex-wrap gap-1">
                                {job.labels.map((label) => (
                                    <span
                                        key={label}
                                        className="inline-flex items-center rounded border border-blue-500/30 bg-blue-500/10 px-2 py-0.5 font-mono text-xs text-blue-400"
                                    >
                                        {label}
                                    </span>
                                ))}
                            </div>
                        </div>
                    )}
                </div>
            </motion.div>

            {/* Timeline Card */}
            <motion.div
                className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.4 }}
            >
                <h2 className="mb-4 font-mono text-sm font-medium text-white">
                    Job Timeline
                </h2>
                <div className="space-y-3">
                    {/* Created Step */}
                    <div className="flex gap-3">
                        <div className="flex flex-shrink-0 flex-col items-center">
                            <div className="flex size-6 flex-shrink-0 items-center justify-center rounded-full bg-blue-500/20">
                                <RiCheckLine className="size-3 text-blue-400" />
                            </div>
                            {(job.queued > 0 ||
                                job.started > 0 ||
                                job.completed > 0) && (
                                <div className="my-1 h-8 w-0.5 bg-white/10" />
                            )}
                        </div>
                        <div className="min-w-0 flex-1 pt-0.5">
                            <p className="font-mono text-xs font-medium text-white">
                                Created
                            </p>
                            <p className="mt-0.5 font-mono text-xs text-white/50">
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
                            <div className="flex flex-shrink-0 flex-col items-center">
                                <div className="flex size-6 flex-shrink-0 items-center justify-center rounded-full bg-blue-500/20">
                                    <RiCheckLine className="size-3 text-blue-400" />
                                </div>
                                {(job.started > 0 || job.completed > 0) && (
                                    <div className="my-1 h-8 w-0.5 bg-white/10" />
                                )}
                            </div>
                            <div className="min-w-0 flex-1 pt-0.5">
                                <div className="flex items-center justify-between gap-2">
                                    <p className="font-mono text-xs font-medium text-white">
                                        Queued
                                    </p>
                                    <span className="flex-shrink-0 font-mono text-xs text-white/50">
                                        +{formatDuration(job.created, job.queued)}
                                    </span>
                                </div>
                                <p className="mt-0.5 font-mono text-xs text-white/50">
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
                            <div className="flex flex-shrink-0 flex-col items-center">
                                <div className="flex size-6 flex-shrink-0 items-center justify-center rounded-full bg-blue-500/20">
                                    <RiCheckLine className="size-3 text-blue-400" />
                                </div>
                                {job.completed > 0 && (
                                    <div className="my-1 h-8 w-0.5 bg-white/10" />
                                )}
                            </div>
                            <div className="min-w-0 flex-1 pt-0.5">
                                <div className="flex items-center justify-between gap-2">
                                    <p className="font-mono text-xs font-medium text-white">
                                        Started
                                    </p>
                                    <span className="flex-shrink-0 font-mono text-xs text-white/50">
                                        +{formatDuration(job.queued, job.started)}
                                    </span>
                                </div>
                                <p className="mt-0.5 font-mono text-xs text-white/50">
                                    {new Date(job.started * 1000).toLocaleString(
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

                    {/* Completed Step */}
                    {job.completed > 0 && (
                        <div className="flex gap-3">
                            <div className="flex flex-shrink-0 flex-col items-center">
                                <div className="flex size-6 flex-shrink-0 items-center justify-center rounded-full bg-green-500/20">
                                    <RiCheckLine className="size-3 text-green-400" />
                                </div>
                            </div>
                            <div className="min-w-0 flex-1 pt-0.5">
                                <div className="flex items-center justify-between gap-2">
                                    <p className="font-mono text-xs font-medium text-white">
                                        Completed
                                    </p>
                                    <span className="flex-shrink-0 font-mono text-xs text-white/50">
                                        +{formatDuration(job.started, job.completed)}
                                    </span>
                                </div>
                                <p className="mt-0.5 font-mono text-xs text-white/50">
                                    {new Date(job.completed * 1000).toLocaleString(
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
                </div>
            </motion.div>

            {/* Runner Assignment */}
            {job?.runner_name && assignedRunner ? (
                <motion.div
                    className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.5 }}
                >
                    <div className="flex items-start gap-4">
                        <div className="flex-shrink-0 rounded-lg bg-blue-500/20 p-3">
                            <RiServerLine
                                className="size-6 text-blue-400"
                                aria-hidden="true"
                            />
                        </div>
                        <div className="min-w-0 flex-1">
                            <h3 className="mb-4 font-mono text-sm font-medium text-white">
                                Runner Assignment
                            </h3>
                            <div className="space-y-3">
                                <div className="border-b border-white/10 pb-3">
                                    <p className="font-mono text-xs text-white/50">
                                        Runner
                                    </p>
                                    <div className="mt-1 flex items-center gap-2">
                                        <p className="font-mono text-sm text-white">
                                            {assignedRunner.name}
                                        </p>
                                        <span className="inline-flex items-center rounded border border-blue-500/30 bg-blue-500/10 px-2 py-0.5 font-mono text-xs capitalize text-blue-400">
                                            {assignedRunner.status}
                                        </span>
                                    </div>
                                </div>
                                <div className="border-b border-white/10 pb-3">
                                    <p className="font-mono text-xs text-white/50">
                                        Machine
                                    </p>
                                    <button
                                        onClick={handleMachineClick}
                                        className="mt-1 flex items-center gap-1 font-mono text-xs text-blue-400 transition-colors hover:text-blue-300"
                                    >
                                        <span>{assignedRunner.machine}</span>
                                        <RiArrowRightLine className="size-3" />
                                    </button>
                                </div>
                                <div>
                                    <p className="mb-3 font-mono text-xs text-white/50">
                                        Architecture & Resources
                                    </p>
                                    <div className="flex flex-wrap gap-4">
                                        <div className="flex items-center gap-2">
                                            <span className="font-mono text-xs text-white/30">
                                                {assignedRunner.arch === 'x86_64'
                                                    ? 'x86'
                                                    : assignedRunner.arch === 'arm64'
                                                      ? 'ARM'
                                                      : assignedRunner.arch}
                                            </span>
                                            <p className="font-mono text-sm text-white">
                                                {assignedRunner.arch}
                                            </p>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <RiCpuLine
                                                className="size-4 text-blue-400"
                                                aria-hidden="true"
                                            />
                                            <p className="font-mono text-sm text-white">
                                                <span className="font-medium">
                                                    {assignedRunner.cpu}
                                                </span>{' '}
                                                <span className="text-white/50">vCPU</span>
                                            </p>
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <RiRam2Line
                                                className="size-4 text-blue-400"
                                                aria-hidden="true"
                                            />
                                            <p className="font-mono text-sm text-white">
                                                <span className="font-medium">
                                                    {Math.round(assignedRunner.ram / 1024)}
                                                </span>{' '}
                                                <span className="text-white/50">GB RAM</span>
                                            </p>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </motion.div>
            ) : job.runner_name ? (
                <motion.div
                    className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.5 }}
                >
                    <div className="flex items-start gap-4">
                        <div className="flex-shrink-0 rounded-lg bg-blue-500/20 p-3">
                            <RiServerLine
                                className="size-6 text-blue-400"
                                aria-hidden="true"
                            />
                        </div>
                        <div className="flex-1">
                            <h3 className="font-mono text-sm font-medium text-white">
                                Runner
                            </h3>
                            <p className="mt-2 font-mono text-sm text-white">
                                {job.runner_name}
                            </p>
                        </div>
                    </div>
                </motion.div>
            ) : null}
        </div>
    );
}
