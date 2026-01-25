import { StatCard } from '@/components/StatCard';
import { useInstallation } from '@/hooks/useInstallation';
import { useRunnerDetail } from '@/hooks/useRunners';
import { cx } from '@/lib/utils';
import type { RunnerStatus } from '@/types/runner';
import {
    RiAlertLine,
    RiArrowLeftLine,
    RiCheckboxCircleLine,
    RiCloseLine,
    RiCpuLine,
    RiExternalLinkLine,
    RiGitBranchLine,
    RiGitCommitLine,
    RiLoader4Line,
    RiPlayCircleLine,
    RiPriceTag3Line,
    RiRam2Line,
    RiServerLine,
    RiStopCircleLine,
    RiTerminalLine,
    RiTimeLine,
} from '@remixicon/react';
import { Link, useNavigate, useParams } from '@tanstack/react-router';
import { motion, AnimatePresence } from 'framer-motion';
import { useState } from 'react';

// Lifecycle stages for a runner
type LifecycleStage = 'created' | 'accepted' | 'started' | 'stopped';

interface LifecycleStep {
    id: LifecycleStage;
    label: string;
    timestamp: number;
}

export function RunnerDetailPage() {
    const navigate = useNavigate();
    const { name: runnerName, login } = useParams({
        from: '/$login/runners/$name',
    });
    const { selectedInstallation } = useInstallation();
    const { data: runner, isLoading, error } = useRunnerDetail(runnerName);
    const [showCancelModal, setShowCancelModal] = useState(false);

    const currentLogin = login || selectedInstallation?.login;

    const formatDate = (timestamp: number) => {
        if (timestamp === 0) return 'N/A';
        return new Date(timestamp * 1000).toLocaleString('en-US', {
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
    };

    const formatDuration = (startTimestamp: number, endTimestamp?: number) => {
        if (startTimestamp === 0) return '-';
        const end = endTimestamp && endTimestamp > 0 ? endTimestamp : Date.now() / 1000;
        const diff = end - startTimestamp;
        if (diff < 60) return `${Math.floor(diff)}s`;
        if (diff < 3600) return `${Math.floor(diff / 60)}m ${Math.floor(diff % 60)}s`;
        return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
    };

    const formatRelativeTime = (timestamp: number) => {
        if (timestamp === 0) return '';
        const now = Date.now() / 1000;
        const diff = now - timestamp;
        if (diff < 60) return 'Just now';
        if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
        if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
        return `${Math.floor(diff / 86400)}d ago`;
    };

    const getStatusConfig = (status: RunnerStatus) => {
        switch (status) {
            case 'busy':
                return {
                    icon: RiPlayCircleLine,
                    color: 'text-emerald-400',
                    bg: 'bg-emerald-500/10',
                    border: 'border-emerald-500/20',
                    label: 'Busy',
                    description: 'Currently executing a workflow job',
                };
            case 'idle':
                return {
                    icon: RiCheckboxCircleLine,
                    color: 'text-blue-400',
                    bg: 'bg-blue-500/10',
                    border: 'border-blue-500/20',
                    label: 'Idle',
                    description: 'Ready and waiting for jobs',
                };
            case 'pending':
                return {
                    icon: RiLoader4Line,
                    color: 'text-amber-400',
                    bg: 'bg-amber-500/10',
                    border: 'border-amber-500/20',
                    label: 'Pending',
                    description: 'Waiting to be registered',
                };
            case 'completed':
                return {
                    icon: RiStopCircleLine,
                    color: 'text-white/40',
                    bg: 'bg-white/5',
                    border: 'border-white/10',
                    label: 'Completed',
                    description: 'Finished and no longer active',
                };
            default:
                return {
                    icon: RiServerLine,
                    color: 'text-white/40',
                    bg: 'bg-white/5',
                    border: 'border-white/10',
                    label: status,
                    description: '',
                };
        }
    };

    const handleCancelRunner = () => {
        // TODO: Implement actual cancel API call
        console.log('Cancelling runner:', runnerName);
        setShowCancelModal(false);
    };

    if (isLoading) {
        return (
            <div className="space-y-8">
                <BackButton
                    onClick={() =>
                        navigate({
                            to: '/$login/runners',
                            params: { login: currentLogin || 'org' },
                        })
                    }
                />
                <div className="h-12 w-64 animate-pulse rounded-lg bg-white/[0.02]" />
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    {[...Array(4)].map((_, i) => (
                        <div
                            key={i}
                            className="h-32 animate-pulse rounded-xl bg-white/[0.02]"
                        />
                    ))}
                </div>
                <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
                    <div className="h-64 animate-pulse rounded-xl bg-white/[0.02] lg:col-span-2" />
                    <div className="h-64 animate-pulse rounded-xl bg-white/[0.02]" />
                </div>
            </div>
        );
    }

    if (error || !runner) {
        return (
            <div className="space-y-4">
                <BackButton
                    onClick={() =>
                        navigate({
                            to: '/$login/runners',
                            params: { login: currentLogin || 'org' },
                        })
                    }
                />
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-6">
                    <p className="font-mono text-sm text-red-400">
                        {error
                            ? 'Failed to load runner details. Please try again later.'
                            : 'Runner not found.'}
                    </p>
                </div>
            </div>
        );
    }

    const statusConfig = getStatusConfig(runner.status);

    // Build lifecycle steps based on runner timestamps
    const lifecycleSteps: LifecycleStep[] = [
        { id: 'created', label: 'Created', timestamp: runner.created },
        { id: 'accepted', label: 'Accepted', timestamp: runner.accepted },
        { id: 'started', label: 'Running', timestamp: runner.started },
        { id: 'stopped', label: 'Completed', timestamp: runner.stopped },
    ];

    // Determine current stage index
    const getCurrentStageIndex = () => {
        if (runner.stopped > 0) return 3;
        if (runner.started > 0) return 2;
        if (runner.accepted > 0) return 1;
        return 0;
    };
    const currentStageIndex = getCurrentStageIndex();

    // Calculate total duration
    const getTotalDuration = () => {
        if (runner.stopped > 0 && runner.created > 0) {
            return formatDuration(runner.created, runner.stopped);
        }
        if (runner.created > 0) {
            return formatDuration(runner.created);
        }
        return '-';
    };

    // Calculate job duration (started to stopped)
    const getJobDuration = () => {
        if (runner.started > 0) {
            if (runner.stopped > 0) {
                return formatDuration(runner.started, runner.stopped);
            }
            return formatDuration(runner.started);
        }
        return '-';
    };

    // Check if runner can be cancelled (not completed)
    const canCancel = runner.status !== 'completed';
    const hasJob = runner.status === 'busy' && runner.job;

    return (
        <>
            <motion.div
                className="space-y-8"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                {/* Back Button */}
                <BackButton
                    onClick={() =>
                        navigate({
                            to: '/$login/runners',
                            params: { login: currentLogin || 'org' },
                        })
                    }
                />

                {/* Header Section */}
                <div className="space-y-4">
                    {/* Title Row */}
                    <div className="flex items-start justify-between gap-4">
                        <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-3">
                                <h1 className="truncate font-display text-3xl text-white">
                                    {runner.name}
                                </h1>
                                <span
                                    className={cx(
                                        'inline-flex items-center gap-1.5 rounded-full border px-3 py-1',
                                        statusConfig.bg,
                                        statusConfig.border,
                                    )}
                                >
                                    <statusConfig.icon
                                        className={cx(
                                            'size-4',
                                            statusConfig.color,
                                            runner.status === 'pending' && 'animate-spin',
                                        )}
                                    />
                                    <span className={cx('font-mono text-sm', statusConfig.color)}>
                                        {statusConfig.label}
                                    </span>
                                </span>
                            </div>
                        </div>
                        {/* Cancel Button */}
                        {canCancel && (
                            <motion.button
                                onClick={() => setShowCancelModal(true)}
                                className="flex items-center gap-2 rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-2 font-mono text-sm text-red-400 transition-colors hover:border-red-500/30 hover:bg-red-500/20"
                                initial={{ opacity: 0, scale: 0.9 }}
                                animate={{ opacity: 1, scale: 1 }}
                                transition={{ delay: 0.2 }}
                            >
                                <RiStopCircleLine className="size-4" />
                                Cancel Runner
                            </motion.button>
                        )}
                    </div>

                    {/* Meta Info Row - Hardware, Labels, Machine */}
                    <div className="flex flex-wrap items-center gap-3">
                        {/* Machine Link */}
                        <Link
                            to="/$login/machines/$name"
                            params={{ login: currentLogin || 'org', name: runner.machine }}
                            className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 font-mono text-xs text-white/70 transition-colors hover:border-white/20 hover:bg-white/10 hover:text-white"
                        >
                            <RiServerLine className="size-3.5" />
                            {runner.machine}
                        </Link>

                        {/* Architecture */}
                        <span className="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 font-mono text-xs text-white/50">
                            <RiTerminalLine className="size-3.5" />
                            {runner.arch}
                        </span>

                        {/* CPU */}
                        <span className="flex items-center gap-1.5 rounded-lg border border-purple-500/20 bg-purple-500/10 px-3 py-1.5 font-mono text-xs text-purple-400">
                            <RiCpuLine className="size-3.5" />
                            {runner.cpu} vCPU
                        </span>

                        {/* RAM */}
                        <span className="flex items-center gap-1.5 rounded-lg border border-orange-500/20 bg-orange-500/10 px-3 py-1.5 font-mono text-xs text-orange-400">
                            <RiRam2Line className="size-3.5" />
                            {Math.round(runner.ram / 1024)} GB
                        </span>

                        {/* Labels */}
                        {runner.labels && runner.labels.length > 0 && (
                            <>
                                <span className="text-white/20">|</span>
                                {runner.labels.map((label) => (
                                    <span
                                        key={label}
                                        className="flex items-center gap-1 rounded-lg border border-blue-500/20 bg-blue-500/10 px-2.5 py-1.5 font-mono text-xs text-blue-400"
                                    >
                                        <RiPriceTag3Line className="size-3" />
                                        {label}
                                    </span>
                                ))}
                            </>
                        )}
                    </div>
                </div>

                {/* Horizontal Lifecycle Timeline */}
                <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                    <div className="relative flex items-start justify-between px-4">
                        {/* Background line */}
                        <div className="absolute left-4 right-4 top-6 h-0.5 bg-white/10" />

                        {/* Progress line */}
                        <div
                            className="absolute left-4 top-6 h-0.5 bg-gradient-to-r from-cyan-500 to-emerald-500 transition-all duration-700 ease-out"
                            style={{
                                width: `calc(${(currentStageIndex / (lifecycleSteps.length - 1)) * 100}% - 32px)`
                            }}
                        />

                        {/* Steps */}
                        {lifecycleSteps.map((step, index) => {
                            const isCompleted = index <= currentStageIndex;
                            const isCurrent = index === currentStageIndex;

                            return (
                                <div
                                    key={step.id}
                                    className="relative z-10 flex flex-col items-center"
                                >
                                    {/* Step indicator */}
                                    <div
                                        className={cx(
                                            'relative flex size-12 items-center justify-center rounded-full border-2 bg-[#0a0a0c] transition-colors duration-300',
                                            isCompleted
                                                ? 'border-emerald-500'
                                                : 'border-white/20',
                                        )}
                                    >
                                        {isCompleted ? (
                                            <RiCheckboxCircleLine className="size-6 text-emerald-400" />
                                        ) : (
                                            <span className="font-mono text-sm text-white/30">
                                                {index + 1}
                                            </span>
                                        )}

                                        {/* Pulse animation for current step */}
                                        {isCurrent && runner.status !== 'completed' && (
                                            <span className="absolute inset-0 animate-ping rounded-full border-2 border-emerald-500 opacity-75" />
                                        )}
                                    </div>

                                    {/* Label and timestamp */}
                                    <div className="mt-3 text-center">
                                        <p className={cx(
                                            'font-mono text-sm font-medium transition-colors duration-300',
                                            isCompleted ? 'text-white' : 'text-white/40',
                                        )}>
                                            {step.label}
                                        </p>
                                        {step.timestamp > 0 ? (
                                            <p className="mt-1 font-mono text-xs text-white/40">
                                                {formatDate(step.timestamp)}
                                            </p>
                                        ) : (
                                            <p className="mt-1 font-mono text-xs text-white/20">
                                                Pending
                                            </p>
                                        )}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </div>

                {/* Stats Overview */}
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    <StatCard
                        label="Status"
                        value={statusConfig.label}
                        subValue={statusConfig.description}
                        icon={statusConfig.icon}
                        iconColor={statusConfig.color}
                        iconBgColor={statusConfig.bg.replace('/10', '/20')}
                        delay={0}
                    />
                    <StatCard
                        label="Total Duration"
                        value={getTotalDuration()}
                        subValue={runner.created > 0 ? `Since ${formatRelativeTime(runner.created)}` : 'Not started'}
                        icon={RiTimeLine}
                        iconColor="text-blue-400"
                        iconBgColor="bg-blue-500/20"
                        delay={0.1}
                    />
                    <StatCard
                        label="Job Duration"
                        value={getJobDuration()}
                        subValue={runner.started > 0 ? 'Execution time' : 'Not running yet'}
                        icon={RiPlayCircleLine}
                        iconColor="text-emerald-400"
                        iconBgColor="bg-emerald-500/20"
                        delay={0.2}
                    />
                    <StatCard
                        label="Runner ID"
                        value={`#${runner.id}`}
                        subValue={`Group ${runner.group_id} • ${runner.owner}`}
                        icon={RiServerLine}
                        iconColor="text-cyan-400"
                        iconBgColor="bg-cyan-500/20"
                        delay={0.3}
                    />
                </div>

                {/* Main Content */}
                <div className="space-y-6">
                    {/* Current Job Card */}
                    {runner.status === 'busy' && runner.job && (
                        <motion.div
                            className="rounded-xl border border-emerald-500/20 bg-emerald-500/5"
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.2 }}
                        >
                            {/* Job Header */}
                            <div className="flex items-center justify-between border-b border-emerald-500/10 px-6 py-4">
                                <div className="flex items-center gap-3">
                                    <div className="flex size-10 items-center justify-center rounded-full bg-emerald-500/20">
                                        <RiPlayCircleLine className="size-5 text-emerald-400" />
                                    </div>
                                    <div>
                                        <h2 className="font-mono text-sm text-white">
                                            {runner.job.name}
                                        </h2>
                                        <p className="font-mono text-xs text-white/50">
                                            {runner.job.workflow}
                                        </p>
                                    </div>
                                </div>
                                <div className="flex items-center gap-3">
                                    <span className="flex items-center gap-1.5 rounded-full bg-emerald-500/10 px-3 py-1">
                                        <span className="size-1.5 animate-pulse rounded-full bg-emerald-500" />
                                        <span className="font-mono text-xs text-emerald-400">Running</span>
                                    </span>
                                    <span className="font-mono text-lg text-emerald-400">
                                        {formatDuration(runner.job.started_at)}
                                    </span>
                                </div>
                            </div>

                            {/* Job Details */}
                            <div className="grid grid-cols-2 gap-4 p-6 sm:grid-cols-4">
                                <div className="flex items-center gap-2">
                                    <RiServerLine className="size-4 text-white/40" />
                                    <div>
                                        <p className="font-mono text-[10px] uppercase text-white/40">Repository</p>
                                        <p className="font-mono text-sm text-white/70">{runner.job.repo}</p>
                                    </div>
                                </div>
                                <div className="flex items-center gap-2">
                                    <RiGitBranchLine className="size-4 text-white/40" />
                                    <div>
                                        <p className="font-mono text-[10px] uppercase text-white/40">Branch</p>
                                        <p className="font-mono text-sm text-white/70">{runner.job.branch}</p>
                                    </div>
                                </div>
                                <div className="flex items-center gap-2">
                                    <RiGitCommitLine className="size-4 text-white/40" />
                                    <div>
                                        <p className="font-mono text-[10px] uppercase text-white/40">Commit</p>
                                        <p className="font-mono text-sm text-white/70">{runner.job.sha.substring(0, 7)}</p>
                                    </div>
                                </div>
                                <div className="flex items-center gap-2">
                                    <img
                                        src={runner.job.author_avatar}
                                        alt={runner.job.author_login}
                                        className="size-6 rounded-full border border-white/10"
                                    />
                                    <div>
                                        <p className="font-mono text-[10px] uppercase text-white/40">Author</p>
                                        <p className="font-mono text-sm text-white/70">{runner.job.author_login}</p>
                                    </div>
                                </div>
                            </div>

                            {/* View on GitHub */}
                            <div className="border-t border-emerald-500/10 px-6 py-3">
                                <a
                                    href={`https://github.com/${runner.owner}/${runner.job.repo}/actions/runs/${runner.job.id}`}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1.5 font-mono text-xs text-white/50 transition-colors hover:text-white"
                                >
                                    <RiExternalLinkLine className="size-3.5" />
                                    View on GitHub
                                </a>
                            </div>
                        </motion.div>
                    )}

                    {/* No Job State */}
                    {runner.status !== 'busy' && (
                        <motion.div
                            className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center"
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.2 }}
                        >
                            <div className="mx-auto mb-4 flex size-14 items-center justify-center rounded-full bg-white/5">
                                <statusConfig.icon
                                    className={cx('size-7', statusConfig.color)}
                                />
                            </div>
                            <h3 className="font-display text-lg text-white">
                                {runner.status === 'idle'
                                    ? 'Waiting for jobs'
                                    : runner.status === 'pending'
                                      ? 'Runner is starting up'
                                      : 'Runner has finished'}
                            </h3>
                            <p className="mt-2 font-mono text-sm text-white/50">
                                {runner.status === 'idle'
                                    ? 'This runner is ready and waiting to pick up the next available job.'
                                    : runner.status === 'pending'
                                      ? 'The runner is being registered and will be ready shortly.'
                                      : 'This runner has completed its work and is no longer active.'}
                            </p>
                        </motion.div>
                    )}

                    {/* Logs Section Placeholder - Could be added */}
                    {runner.status === 'busy' && (
                        <motion.div
                            className="rounded-xl border border-white/10 bg-white/[0.02]"
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.3 }}
                        >
                            <div className="flex items-center justify-between border-b border-white/10 px-6 py-4">
                                <div className="flex items-center gap-2">
                                    <RiTerminalLine className="size-4 text-white/40" />
                                    <h3 className="font-mono text-sm text-white">Live Logs</h3>
                                </div>
                                <span className="flex items-center gap-1.5 rounded bg-white/5 px-2 py-1 font-mono text-[10px] text-white/40">
                                    <span className="size-1.5 animate-pulse rounded-full bg-emerald-500" />
                                    Streaming
                                </span>
                            </div>
                            <div className="h-64 overflow-auto bg-black/20 p-4 font-mono text-xs">
                                <div className="space-y-1 text-white/50">
                                    <p><span className="text-white/30">[{formatDate(runner.started)}]</span> Starting runner...</p>
                                    <p><span className="text-white/30">[{formatDate(runner.started)}]</span> Pulling docker image...</p>
                                    <p><span className="text-white/30">[{formatDate(runner.started)}]</span> <span className="text-emerald-400">✓</span> Image pulled successfully</p>
                                    <p><span className="text-white/30">[{formatDate(runner.started)}]</span> Setting up workspace...</p>
                                    <p><span className="text-white/30">[{formatDate(runner.started)}]</span> <span className="text-emerald-400">✓</span> Workspace ready</p>
                                    <p><span className="text-white/30">[{formatDate(runner.started)}]</span> Running job: {runner.job?.name}</p>
                                    <p className="text-white/30">...</p>
                                </div>
                            </div>
                        </motion.div>
                    )}
                </div>
            </motion.div>

            {/* Cancel Confirmation Modal */}
            <AnimatePresence>
                {showCancelModal && (
                    <motion.div
                        className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        onClick={() => setShowCancelModal(false)}
                    >
                        <motion.div
                            className="mx-4 w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6"
                            initial={{ scale: 0.9, opacity: 0 }}
                            animate={{ scale: 1, opacity: 1 }}
                            exit={{ scale: 0.9, opacity: 0 }}
                            onClick={(e) => e.stopPropagation()}
                        >
                            {/* Modal Header */}
                            <div className="mb-4 flex items-start justify-between">
                                <div className="flex items-center gap-3">
                                    <div className={cx(
                                        'flex size-10 items-center justify-center rounded-full',
                                        hasJob ? 'bg-red-500/20' : 'bg-amber-500/20'
                                    )}>
                                        {hasJob ? (
                                            <RiAlertLine className="size-5 text-red-400" />
                                        ) : (
                                            <RiStopCircleLine className="size-5 text-amber-400" />
                                        )}
                                    </div>
                                    <h3 className="font-display text-lg text-white">
                                        Cancel Runner
                                    </h3>
                                </div>
                                <button
                                    onClick={() => setShowCancelModal(false)}
                                    className="rounded-lg p-1 text-white/40 transition-colors hover:bg-white/5 hover:text-white"
                                >
                                    <RiCloseLine className="size-5" />
                                </button>
                            </div>

                            {/* Modal Content */}
                            <div className="mb-6">
                                {hasJob ? (
                                    <>
                                        <div className="mb-4 rounded-lg border border-red-500/20 bg-red-500/10 p-4">
                                            <div className="flex items-start gap-3">
                                                <RiAlertLine className="mt-0.5 size-5 shrink-0 text-red-400" />
                                                <div>
                                                    <p className="font-mono text-sm text-red-400">
                                                        Warning: Active job will be cancelled
                                                    </p>
                                                    <p className="mt-1 font-mono text-xs text-red-400/70">
                                                        The job "{runner.job?.name}" is currently running and will be terminated.
                                                    </p>
                                                </div>
                                            </div>
                                        </div>
                                        <p className="font-mono text-sm text-white/60">
                                            Are you sure you want to cancel this runner? This action cannot be undone and the job will need to be re-triggered.
                                        </p>
                                    </>
                                ) : (
                                    <p className="font-mono text-sm text-white/60">
                                        Are you sure you want to cancel this runner? The runner is currently {runner.status} and no job is assigned.
                                    </p>
                                )}
                            </div>

                            {/* Modal Actions */}
                            <div className="flex gap-3">
                                <button
                                    onClick={() => setShowCancelModal(false)}
                                    className="flex-1 rounded-lg border border-white/10 bg-white/5 px-4 py-2 font-mono text-sm text-white/70 transition-colors hover:bg-white/10 hover:text-white"
                                >
                                    Keep Running
                                </button>
                                <button
                                    onClick={handleCancelRunner}
                                    className={cx(
                                        'flex-1 rounded-lg px-4 py-2 font-mono text-sm transition-colors',
                                        hasJob
                                            ? 'bg-red-500/20 text-red-400 hover:bg-red-500/30'
                                            : 'bg-amber-500/20 text-amber-400 hover:bg-amber-500/30'
                                    )}
                                >
                                    {hasJob ? 'Cancel Runner & Job' : 'Cancel Runner'}
                                </button>
                            </div>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>
        </>
    );
}

function BackButton({ onClick }: { onClick: () => void }) {
    return (
        <button
            onClick={onClick}
            className="inline-flex items-center gap-2 font-mono text-sm text-white/60 transition-colors hover:text-white"
        >
            <RiArrowLeftLine className="size-4" />
            Back to Runners
        </button>
    );
}
