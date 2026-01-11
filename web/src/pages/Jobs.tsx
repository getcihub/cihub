import { JobStatusBadge } from '@/components/JobStatusBadge';
import { useInstallation } from '@/hooks/useInstallation';
import { useJobs } from '@/hooks/useJobs';
import {
    JobStatusCompleted,
    JobStatusInProgress,
    JobStatusQueued,
    JobStatusWaiting,
} from '@/types/job';
import { RiArrowRightLine, RiBriefcaseLine } from '@remixicon/react';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useEffect, useState } from 'react';

const JOBS_PAGE_SIZE = 25;

export function JobsPage() {
    const navigate = useNavigate();
    const { selectedInstallation } = useInstallation();
    const searchParams = useSearch({ from: '/$login/jobs' }) as
        | { tab?: string }
        | undefined;
    const initialTab =
        (searchParams?.tab as 'incomplete' | 'completed') || 'incomplete';

    const [activeTab, setActiveTab] = useState<'incomplete' | 'completed'>(
        initialTab,
    );
    const [completedCursor, setCompletedCursor] = useState<number>(0);

    // Update URL when tab changes
    useEffect(() => {
        navigate({
            to: '/$login/jobs',
            params: { login: selectedInstallation!.login },
            search: { tab: activeTab },
            replace: true,
        });
    }, [activeTab, navigate, selectedInstallation]);

    // Fetch incomplete jobs
    const {
        data: incompleteData = { data: [], hasMore: false },
        isLoading: incompleteLoading,
        error: incompleteError,
    } = useJobs({ status: 'incomplete' });

    // Fetch completed jobs with pagination
    const {
        data: completedData = { data: [], hasMore: false },
        isLoading: completedLoading,
        error: completedError,
    } = useJobs({
        status: 'completed',
        limit: JOBS_PAGE_SIZE,
        jobId: completedCursor,
    });

    const incompleteJobs = incompleteData.data || [];
    const completedJobs = completedData.data || [];

    // Filter incomplete jobs by status for statistics
    const queuedJobs = incompleteJobs.filter(
        (j) => j.status === JobStatusQueued,
    );
    const waitingJobs = incompleteJobs.filter(
        (j) => j.status === JobStatusWaiting,
    );
    const inProgressJobs = incompleteJobs.filter(
        (j) => j.status === JobStatusInProgress,
    );

    // Sort incomplete jobs by status priority: in_progress first, then queued, then waiting
    const statusPriority: Record<string, number> = {
        [JobStatusInProgress]: 0,
        [JobStatusQueued]: 1,
        [JobStatusWaiting]: 2,
        [JobStatusCompleted]: 3,
    };

    const sortedIncompleteJobs = [...incompleteJobs].sort((a, b) => {
        const priorityA = statusPriority[a.status] ?? 999;
        const priorityB = statusPriority[b.status] ?? 999;
        return priorityA - priorityB;
    });

    // Calculate statistics
    const totalJobs = incompleteJobs.length;
    const activeJobs = inProgressJobs.length + waitingJobs.length;

    const handleJobClick = (jobId: string) => {
        navigate({
            to: '/$login/jobs/$jobId',
            params: { login: selectedInstallation!.login, jobId },
        });
    };

    const handleLoadMoreCompleted = () => {
        if (completedJobs.length > 0) {
            const lastJobId = completedJobs[completedJobs.length - 1].id;
            setCompletedCursor(lastJobId);
        }
    };

    const isLoading =
        activeTab === 'incomplete' ? incompleteLoading : completedLoading;
    const error = activeTab === 'incomplete' ? incompleteError : completedError;
    const hasMore = activeTab === 'incomplete' ? false : completedData.hasMore;

    if (isLoading) {
        return (
            <div className="space-y-8">
                <div>
                    <h1 className="font-display text-3xl text-white">Jobs</h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Manage and monitor your job queue
                    </p>
                </div>
                <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                    {[...Array(3)].map((_, i) => (
                        <div
                            key={i}
                            className="h-32 animate-pulse rounded-xl border border-white/10 bg-white/[0.02]"
                        />
                    ))}
                </div>
                <div className="space-y-4">
                    {[...Array(5)].map((_, i) => (
                        <div
                            key={i}
                            className="h-16 animate-pulse rounded-xl border border-white/10 bg-white/[0.02]"
                        />
                    ))}
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="space-y-4">
                <h1 className="font-display text-3xl text-white">Jobs</h1>
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-6">
                    <p className="font-mono text-sm text-red-400">
                        Failed to load jobs. Please try again later.
                    </p>
                </div>
            </div>
        );
    }

    return (
        <div className="space-y-8">
            <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                <h1 className="font-display text-3xl text-white">Jobs</h1>
                <p className="mt-2 font-mono text-sm text-white/50">
                    Manage and monitor your job queue
                </p>
            </motion.div>

            {/* Filter Bar */}
            <motion.div
                className="flex gap-2 overflow-x-auto pb-2"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.1 }}
            >
                {(['incomplete', 'completed'] as const).map((status) => (
                    <button
                        key={status}
                        onClick={() => {
                            setActiveTab(status);
                            if (status === 'completed') {
                                setCompletedCursor(0);
                            }
                        }}
                        className={`whitespace-nowrap rounded-lg px-4 py-2 font-mono text-sm transition-all ${
                            activeTab === status
                                ? 'bg-white text-black'
                                : 'border border-white/10 bg-white/[0.02] text-white/70 hover:border-white/20 hover:bg-white/5 hover:text-white'
                        }`}
                    >
                        <span>
                            {status === 'incomplete' ? 'Active' : 'Completed'}{' '}
                            Jobs
                        </span>
                    </button>
                ))}
            </motion.div>

            {/* Incomplete Jobs Section */}
            {activeTab === 'incomplete' && (
                <>
                    {/* Statistics Cards */}
                    <motion.div
                        className="grid grid-cols-1 gap-4 md:grid-cols-3"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.2 }}
                    >
                        <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                            <div className="flex items-start justify-between">
                                <div>
                                    <p className="font-mono text-sm text-white/50">
                                        Total Active Jobs
                                    </p>
                                    <div className="mt-2">
                                        <p className="font-display text-3xl text-white">
                                            {totalJobs}
                                        </p>
                                        <p className="mt-1 font-mono text-xs text-white/30">
                                            {activeJobs} running or waiting
                                        </p>
                                    </div>
                                </div>
                                <div className="rounded-lg bg-blue-500/20 p-3">
                                    <RiBriefcaseLine
                                        className="size-6 text-blue-400"
                                        aria-hidden="true"
                                    />
                                </div>
                            </div>
                        </div>

                        <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                            <div className="flex items-start justify-between">
                                <div>
                                    <p className="font-mono text-sm text-white/50">
                                        In Progress
                                    </p>
                                    <div className="mt-2">
                                        <p className="font-display text-3xl text-white">
                                            {inProgressJobs.length}
                                        </p>
                                        <p className="mt-1 font-mono text-xs text-white/30">
                                            Currently running
                                        </p>
                                    </div>
                                </div>
                                <div className="rounded-lg bg-green-500/20 p-3">
                                    <RiBriefcaseLine
                                        className="size-6 text-green-400"
                                        aria-hidden="true"
                                    />
                                </div>
                            </div>
                        </div>

                        <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                            <div className="flex items-start justify-between">
                                <div>
                                    <p className="font-mono text-sm text-white/50">
                                        Queued
                                    </p>
                                    <div className="mt-2">
                                        <p className="font-display text-3xl text-white">
                                            {queuedJobs.length}
                                        </p>
                                        <p className="mt-1 font-mono text-xs text-white/30">
                                            Waiting to run
                                        </p>
                                    </div>
                                </div>
                                <div className="rounded-lg bg-yellow-500/20 p-3">
                                    <RiBriefcaseLine
                                        className="size-6 text-yellow-400"
                                        aria-hidden="true"
                                    />
                                </div>
                            </div>
                        </div>
                    </motion.div>

                    {/* Incomplete Jobs List */}
                    <motion.div
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.3 }}
                    >
                        {incompleteJobs.length === 0 ? (
                            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center">
                                <RiBriefcaseLine
                                    className="mx-auto mb-4 size-12 text-white/20"
                                    aria-hidden="true"
                                />
                                <p className="mb-2 font-display text-lg text-white">
                                    No active jobs
                                </p>
                                <p className="font-mono text-sm text-white/50">
                                    All jobs have been completed. Trigger a
                                    workflow to get started.
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-3">
                                {sortedIncompleteJobs.map((job, index) => (
                                    <motion.div
                                        key={job.id}
                                        initial={{ opacity: 0, y: 10 }}
                                        animate={{ opacity: 1, y: 0 }}
                                        transition={{
                                            duration: 0.3,
                                            delay: 0.3 + index * 0.05,
                                        }}
                                        onClick={() =>
                                            handleJobClick(String(job.id))
                                        }
                                        className="cursor-pointer rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-all hover:border-white/20 hover:bg-white/5"
                                    >
                                        <div className="flex items-center justify-between gap-3">
                                            <div className="flex min-w-0 flex-1 items-center gap-3">
                                                {/* Author Avatar */}
                                                <img
                                                    src={job.author_avatar}
                                                    alt={job.author_login}
                                                    className="size-8 flex-shrink-0 rounded-full border border-white/10 object-cover"
                                                    title={job.author_login}
                                                />
                                                {/* Job Info */}
                                                <div className="min-w-0 flex-1">
                                                    <p className="truncate font-mono text-sm font-medium text-white">
                                                        {job.name}
                                                    </p>
                                                    <div className="mt-1 flex items-center gap-2">
                                                        <p className="truncate font-mono text-xs text-white/50">
                                                            {job.owner}/
                                                            {job.repo}
                                                        </p>
                                                        <span className="text-white/30">
                                                            •
                                                        </span>
                                                        <p className="truncate font-mono text-xs text-white/50">
                                                            {job.branch}
                                                        </p>
                                                        <span className="text-white/30">
                                                            •
                                                        </span>
                                                        <p className="truncate font-mono text-xs text-white/50">
                                                            {job.sha.substring(
                                                                0,
                                                                7,
                                                            )}
                                                        </p>
                                                    </div>
                                                    <p className="mt-0.5 truncate font-mono text-xs text-white/30">
                                                        {job.workflow} /{' '}
                                                        {job.name}
                                                    </p>
                                                </div>
                                            </div>
                                            {/* Status Badge */}
                                            <div className="flex-shrink-0">
                                                <JobStatusBadge
                                                    status={job.status}
                                                    size="sm"
                                                />
                                            </div>
                                        </div>
                                    </motion.div>
                                ))}
                            </div>
                        )}
                    </motion.div>
                </>
            )}

            {/* Completed Jobs Section */}
            {activeTab === 'completed' && (
                <motion.div
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.2 }}
                >
                    {completedJobs.length === 0 ? (
                        <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center">
                            <RiBriefcaseLine
                                className="mx-auto mb-4 size-12 text-white/20"
                                aria-hidden="true"
                            />
                            <p className="mb-2 font-display text-lg text-white">
                                No completed jobs
                            </p>
                            <p className="font-mono text-sm text-white/50">
                                You haven't completed any jobs yet.
                            </p>
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {completedJobs.map((job, index) => (
                                <motion.div
                                    key={job.id}
                                    initial={{ opacity: 0, y: 10 }}
                                    animate={{ opacity: 1, y: 0 }}
                                    transition={{
                                        duration: 0.3,
                                        delay: index * 0.05,
                                    }}
                                    onClick={() =>
                                        handleJobClick(String(job.id))
                                    }
                                    className="cursor-pointer rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-all hover:border-white/20 hover:bg-white/5"
                                >
                                    <div className="flex items-center justify-between gap-3">
                                        <div className="flex min-w-0 flex-1 items-center gap-3">
                                            {/* Author Avatar */}
                                            <img
                                                src={job.author_avatar}
                                                alt={job.author_login}
                                                className="size-8 flex-shrink-0 rounded-full border border-white/10 object-cover"
                                                title={job.author_login}
                                            />
                                            {/* Job Info */}
                                            <div className="min-w-0 flex-1">
                                                <p className="truncate font-mono text-sm font-medium text-white">
                                                    {job.name}
                                                </p>
                                                <div className="mt-1 flex items-center gap-2">
                                                    <p className="truncate font-mono text-xs text-white/50">
                                                        {job.owner}/{job.repo}
                                                    </p>
                                                    <span className="text-white/30">
                                                        •
                                                    </span>
                                                    <p className="truncate font-mono text-xs text-white/50">
                                                        {job.branch}
                                                    </p>
                                                    <span className="text-white/30">
                                                        •
                                                    </span>
                                                    <p className="truncate font-mono text-xs text-white/50">
                                                        {job.sha.substring(
                                                            0,
                                                            7,
                                                        )}
                                                    </p>
                                                </div>
                                                <p className="mt-0.5 truncate font-mono text-xs text-white/30">
                                                    {job.workflow} / {job.name}
                                                </p>
                                            </div>
                                        </div>
                                        {/* Status Badge */}
                                        <div className="flex-shrink-0">
                                            <JobStatusBadge
                                                status={job.status}
                                                size="sm"
                                            />
                                        </div>
                                    </div>
                                </motion.div>
                            ))}

                            {/* Load More Button */}
                            {hasMore && (
                                <button
                                    onClick={handleLoadMoreCompleted}
                                    disabled={completedLoading}
                                    className="flex w-full items-center justify-center gap-2 rounded-xl border border-white/10 bg-white/[0.02] px-4 py-3 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                                >
                                    {completedLoading
                                        ? 'Loading...'
                                        : 'Load More'}
                                    {!completedLoading && (
                                        <RiArrowRightLine className="size-4" />
                                    )}
                                </button>
                            )}
                        </div>
                    )}
                </motion.div>
            )}
        </div>
    );
}
