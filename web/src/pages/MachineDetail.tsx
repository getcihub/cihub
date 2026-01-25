import { ResourceBar } from '@/components/ResourceBar';
import { StatCard } from '@/components/StatCard';
import { useInstallation } from '@/hooks/useInstallation';
import { useMachineDetail } from '@/hooks/useMachineDetail';
import { useMachineMutations } from '@/hooks/useMachineMutations';
import { cx } from '@/lib/utils';
import { MembershipRoleAdmin } from '@/types/installation';
import type { Runner } from '@/types/runner';
import {
    RiAlertLine,
    RiArrowLeftLine,
    RiCalendarLine,
    RiCheckboxCircleLine,
    RiCloseCircleLine,
    RiCpuLine,
    RiDeleteBin6Line,
    RiEditLine,
    RiMoreLine,
    RiPauseLine,
    RiPlayCircleLine,
    RiPriceTag3Line,
    RiRam2Line,
    RiServerLine,
    RiTimeLine,
} from '@remixicon/react';
import { useNavigate, useParams } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';

export function MachineDetailPage() {
    const navigate = useNavigate();
    const { name: machineName, login } = useParams({
        from: '/$login/machines/$name',
    });
    const { selectedInstallation } = useInstallation();
    const { data: machine, isLoading, error } = useMachineDetail(machineName);
    const { pauseMachine, resumeMachine, deleteMachine, updateMachineLimit } =
        useMachineMutations();
    const [showSettings, setShowSettings] = useState(false);
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
    const [showPauseConfirm, setShowPauseConfirm] = useState(false);
    const [showResumeConfirm, setShowResumeConfirm] = useState(false);
    const [showEditLimits, setShowEditLimits] = useState(false);
    const [editCPULimit, setEditCPULimit] = useState<string>('');
    const [editRAMLimit, setEditRAMLimit] = useState<string>('');
    const [limitsError, setLimitsError] = useState<string>('');
    const settingsRef = useRef<HTMLDivElement>(null);

    const isAdmin =
        selectedInstallation?.membership?.role === MembershipRoleAdmin;
    const currentLogin = login || selectedInstallation?.login;

    // Close settings menu when clicking outside
    useEffect(() => {
        function handleClickOutside(event: MouseEvent) {
            if (
                settingsRef.current &&
                !settingsRef.current.contains(event.target as Node)
            ) {
                setShowSettings(false);
            }
        }

        if (showSettings) {
            document.addEventListener('mousedown', handleClickOutside);
            return () => {
                document.removeEventListener('mousedown', handleClickOutside);
            };
        }
    }, [showSettings]);

    // Calculate resource metrics
    const metrics = useMemo(() => {
        if (!machine) return null;

        const cpuLimit =
            machine.cpu_limit > 0 ? machine.cpu_limit : machine.cpu;
        const ramLimit =
            machine.ram_limit > 0 ? machine.ram_limit : machine.ram_total;

        return {
            cpuLimit,
            ramLimit,
            cpuAvailable: cpuLimit - machine.cpu_allocated,
            ramAvailable: ramLimit - machine.ram_allocated,
            cpuPercent:
                cpuLimit > 0
                    ? Math.round((machine.cpu_allocated / cpuLimit) * 100)
                    : 0,
            ramPercent:
                ramLimit > 0
                    ? Math.round((machine.ram_allocated / ramLimit) * 100)
                    : 0,
        };
    }, [machine]);

    const formatDate = (timestamp: number) => {
        if (timestamp === 0) return 'Never';
        return new Date(timestamp * 1000).toLocaleString('en-US', {
            year: 'numeric',
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
    };

    const formatRelativeTime = (timestamp: number) => {
        if (timestamp === 0) return 'Never';
        const now = Date.now() / 1000;
        const diff = now - timestamp;
        if (diff < 60) return 'Just now';
        if (diff < 3600) return `${Math.floor(diff / 60)} minutes ago`;
        if (diff < 86400) return `${Math.floor(diff / 3600)} hours ago`;
        return `${Math.floor(diff / 86400)} days ago`;
    };

    const getStatusConfig = (status: string) => {
        switch (status) {
            case 'online':
                return {
                    icon: RiCheckboxCircleLine,
                    color: 'text-emerald-400',
                    bg: 'bg-emerald-500/10',
                    border: 'border-emerald-500/20',
                    dot: 'bg-emerald-500 shadow-lg shadow-emerald-500/50',
                    label: 'Online',
                };
            case 'paused':
                return {
                    icon: RiPauseLine,
                    color: 'text-amber-400',
                    bg: 'bg-amber-500/10',
                    border: 'border-amber-500/20',
                    dot: 'bg-amber-500',
                    label: 'Paused',
                };
            default:
                return {
                    icon: RiCloseCircleLine,
                    color: 'text-white/40',
                    bg: 'bg-white/5',
                    border: 'border-white/10',
                    dot: 'bg-white/30',
                    label: 'Offline',
                };
        }
    };

    // Handler functions
    const handlePauseMachine = () => {
        setShowPauseConfirm(true);
        setShowSettings(false);
    };

    const handleConfirmPause = async () => {
        try {
            await pauseMachine.mutateAsync(machineName);
            setShowPauseConfirm(false);
            toast.success('Machine paused successfully');
        } catch {
            toast.error('Failed to pause machine');
        }
    };

    const handleResumeMachine = () => {
        setShowResumeConfirm(true);
        setShowSettings(false);
    };

    const handleConfirmResume = async () => {
        try {
            await resumeMachine.mutateAsync(machineName);
            setShowResumeConfirm(false);
            toast.success('Machine resumed successfully');
        } catch {
            toast.error('Failed to resume machine');
        }
    };

    const handleDeleteMachineClick = () => {
        setShowDeleteConfirm(true);
        setShowSettings(false);
    };

    const handleConfirmDelete = async () => {
        try {
            await deleteMachine.mutateAsync(machineName);
            setShowDeleteConfirm(false);
            toast.success(`Machine "${machineName}" deleted successfully`);
            navigate({
                to: '/$login/machines',
                params: { login: currentLogin || 'org' },
            });
        } catch {
            toast.error('Failed to delete machine');
        }
    };

    const handleEditLimitsClick = () => {
        if (machine) {
            setEditCPULimit(
                machine.cpu_limit > 0 ? machine.cpu_limit.toString() : '',
            );
            setEditRAMLimit(
                machine.ram_limit > 0
                    ? Math.round(machine.ram_limit / 1024).toString()
                    : '',
            );
            setLimitsError('');
            setShowEditLimits(true);
            setShowSettings(false);
        }
    };

    const hasChanges =
        machine &&
        (editCPULimit !==
            (machine.cpu_limit > 0 ? machine.cpu_limit.toString() : '') ||
            editRAMLimit !==
                (machine.ram_limit > 0
                    ? Math.round(machine.ram_limit / 1024).toString()
                    : ''));

    const handleConfirmEditLimits = async () => {
        setLimitsError('');

        const cpuLimit = editCPULimit ? parseInt(editCPULimit, 10) : 0;
        const ramLimitGB = editRAMLimit ? parseInt(editRAMLimit, 10) : 0;
        const ramLimitMB = ramLimitGB * 1024;

        if (editCPULimit && isNaN(cpuLimit)) {
            setLimitsError('Invalid CPU limit');
            return;
        }

        if (editRAMLimit && isNaN(ramLimitGB)) {
            setLimitsError('Invalid RAM limit');
            return;
        }

        if (machine) {
            if (machine.cpu > 0 && cpuLimit > 0 && cpuLimit > machine.cpu) {
                setLimitsError(
                    `CPU limit cannot exceed discovered CPU (${machine.cpu} vCPU)`,
                );
                return;
            }

            if (
                machine.ram_total > 0 &&
                ramLimitGB > 0 &&
                ramLimitMB > machine.ram_total
            ) {
                setLimitsError(
                    `RAM limit cannot exceed discovered RAM (${Math.round(machine.ram_total / 1024)} GB)`,
                );
                return;
            }
        }

        try {
            await updateMachineLimit.mutateAsync({
                machineName,
                cpu: cpuLimit,
                ram: ramLimitMB,
            });
            setShowEditLimits(false);
            toast.success('Machine limits updated successfully');
        } catch (error) {
            const errorMsg =
                error instanceof Error
                    ? error.message
                    : 'Failed to update limits';
            setLimitsError(errorMsg);
        }
    };

    if (isLoading) {
        return (
            <div className="space-y-8">
                <BackButton
                    onClick={() =>
                        navigate({
                            to: '/$login/machines',
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

    if (error || !machine) {
        return (
            <div className="space-y-4">
                <BackButton
                    onClick={() =>
                        navigate({
                            to: '/$login/machines',
                            params: { login: currentLogin || 'org' },
                        })
                    }
                />
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-6">
                    <p className="font-mono text-sm text-red-400">
                        {error
                            ? 'Failed to load machine details. Please try again later.'
                            : 'Machine not found.'}
                    </p>
                </div>
            </div>
        );
    }

    const statusConfig = getStatusConfig(machine.status);
    const runners = machine.runners || [];

    return (
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
                        to: '/$login/machines',
                        params: { login: currentLogin || 'org' },
                    })
                }
            />

            {/* Header */}
            <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-3">
                        <h1 className="truncate font-display text-3xl text-white">
                            {machine.name}
                        </h1>
                        <span
                            className={cx(
                                'inline-flex items-center gap-1.5 rounded-full border px-3 py-1',
                                statusConfig.bg,
                                statusConfig.border,
                            )}
                        >
                            <div
                                className={cx(
                                    'size-2 rounded-full',
                                    statusConfig.dot,
                                    machine.status === 'online' && 'animate-pulse',
                                )}
                            />
                            <span
                                className={cx(
                                    'font-mono text-sm',
                                    statusConfig.color,
                                )}
                            >
                                {statusConfig.label}
                            </span>
                        </span>
                    </div>
                    <div className="mt-2 flex items-center gap-4 font-mono text-sm text-white/50">
                        <span className="flex items-center gap-1.5">
                            <RiTimeLine className="size-4" />
                            Last seen {formatRelativeTime(machine.last_seen_at)}
                        </span>
                        <span className="rounded bg-white/5 px-2 py-0.5 text-xs">
                            {machine.arch}
                        </span>
                    </div>
                </div>

                {isAdmin && (
                    <div className="relative flex-shrink-0" ref={settingsRef}>
                        <button
                            onClick={() => setShowSettings(!showSettings)}
                            className="rounded-lg border border-white/10 bg-white/[0.02] p-2.5 transition-colors hover:bg-white/5"
                        >
                            <RiMoreLine className="size-5 text-white/60" />
                        </button>
                        {showSettings && (
                            <SettingsMenu
                                status={machine.status}
                                onEditLimits={handleEditLimitsClick}
                                onPause={handlePauseMachine}
                                onResume={handleResumeMachine}
                                onDelete={handleDeleteMachineClick}
                            />
                        )}
                    </div>
                )}
            </div>

            {/* Stats Overview */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <StatCard
                    label="Active Runners"
                    value={runners.length}
                    subValue={`${runners.filter((r) => r.status === 'busy').length} currently busy`}
                    icon={RiServerLine}
                    iconColor="text-blue-400"
                    iconBgColor="bg-blue-500/20"
                    delay={0}
                />
                <StatCard
                    label="CPU Usage"
                    value={metrics ? `${metrics.cpuPercent}%` : '0%'}
                    subValue={
                        metrics
                            ? `${machine.cpu_allocated} / ${metrics.cpuLimit} vCPU`
                            : 'No data'
                    }
                    icon={RiCpuLine}
                    iconColor="text-purple-400"
                    iconBgColor="bg-purple-500/20"
                    progress={
                        metrics
                            ? { value: metrics.cpuPercent, color: 'bg-purple-500' }
                            : undefined
                    }
                    delay={0.1}
                />
                <StatCard
                    label="Memory Usage"
                    value={metrics ? `${metrics.ramPercent}%` : '0%'}
                    subValue={
                        metrics
                            ? `${Math.round(machine.ram_allocated / 1024)} / ${Math.round(metrics.ramLimit / 1024)} GB`
                            : 'No data'
                    }
                    icon={RiRam2Line}
                    iconColor="text-orange-400"
                    iconBgColor="bg-orange-500/20"
                    progress={
                        metrics
                            ? { value: metrics.ramPercent, color: 'bg-orange-500' }
                            : undefined
                    }
                    delay={0.2}
                />
                <StatCard
                    label="Uptime"
                    value={formatRelativeTime(machine.created_at)}
                    subValue={`Created ${formatDate(machine.created_at)}`}
                    icon={RiCalendarLine}
                    iconColor="text-emerald-400"
                    iconBgColor="bg-emerald-500/20"
                    delay={0.3}
                />
            </div>

            {/* Main Content Grid */}
            <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
                {/* Left Column - Resources & Runners */}
                <div className="space-y-6 lg:col-span-2">
                    {/* Resource Details Card */}
                    <motion.div
                        className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.2 }}
                    >
                        <h2 className="mb-6 font-display text-lg text-white">
                            Resource Allocation
                        </h2>
                        <div className="space-y-6">
                            <ResourceBar
                                label="CPU"
                                icon={RiCpuLine}
                                iconColor="text-purple-400"
                                allocated={machine.cpu_allocated}
                                limit={metrics?.cpuLimit || 0}
                                total={machine.cpu}
                                unit=" vCPU"
                                barColor="bg-purple-500"
                                showDetails
                                size="lg"
                                delay={0.3}
                            />
                            <ResourceBar
                                label="Memory"
                                icon={RiRam2Line}
                                iconColor="text-orange-400"
                                allocated={Math.round(machine.ram_allocated / 1024)}
                                limit={Math.round((metrics?.ramLimit || 0) / 1024)}
                                total={Math.round(machine.ram_total / 1024)}
                                unit=" GB"
                                barColor="bg-orange-500"
                                showDetails
                                size="lg"
                                delay={0.4}
                            />
                        </div>
                    </motion.div>

                    {/* Runners Section */}
                    <motion.div
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.3 }}
                    >
                        <div className="mb-4 flex items-center justify-between">
                            <h2 className="font-display text-lg text-white">
                                Active Runners
                            </h2>
                            <span className="font-mono text-sm text-white/40">
                                {runners.length} runner
                                {runners.length !== 1 ? 's' : ''}
                            </span>
                        </div>

                        {runners.length === 0 ? (
                            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center">
                                <div className="mx-auto mb-4 flex size-14 items-center justify-center rounded-full bg-white/5">
                                    <RiServerLine className="size-7 text-white/30" />
                                </div>
                                <p className="font-display text-white">
                                    No active runners
                                </p>
                                <p className="mt-1 font-mono text-sm text-white/50">
                                    Runners will appear here when jobs are assigned
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-3">
                                {runners.map((runner, index) => (
                                    <RunnerCard
                                        key={runner.id}
                                        runner={runner}
                                        index={index}
                                    />
                                ))}
                            </div>
                        )}
                    </motion.div>
                </div>

                {/* Right Column - Details */}
                <div className="space-y-6">
                    {/* Labels Card */}
                    {machine.labels && machine.labels.length > 0 && (
                        <motion.div
                            className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.25 }}
                        >
                            <div className="mb-4 flex items-center gap-2">
                                <RiPriceTag3Line className="size-4 text-white/40" />
                                <h3 className="font-display text-sm text-white">
                                    Labels
                                </h3>
                            </div>
                            <div className="flex flex-wrap gap-2">
                                {machine.labels.map((label) => (
                                    <span
                                        key={label}
                                        className="rounded-full bg-blue-500/10 px-3 py-1 font-mono text-xs text-blue-400"
                                    >
                                        {label}
                                    </span>
                                ))}
                            </div>
                        </motion.div>
                    )}

                    {/* System Info Card */}
                    <motion.div
                        className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.3 }}
                    >
                        <h3 className="mb-4 font-display text-sm text-white">
                            System Information
                        </h3>
                        <div className="space-y-4">
                            <InfoRow label="Architecture" value={machine.arch} />
                            <InfoRow
                                label="Total CPU"
                                value={
                                    machine.cpu > 0
                                        ? `${machine.cpu} vCPU`
                                        : 'Not detected'
                                }
                            />
                            <InfoRow
                                label="Total Memory"
                                value={
                                    machine.ram_total > 0
                                        ? `${Math.round(machine.ram_total / 1024)} GB`
                                        : 'Not detected'
                                }
                            />
                            {machine.cpu_limit > 0 && (
                                <InfoRow
                                    label="CPU Limit"
                                    value={`${machine.cpu_limit} vCPU`}
                                />
                            )}
                            {machine.ram_limit > 0 && (
                                <InfoRow
                                    label="Memory Limit"
                                    value={`${Math.round(machine.ram_limit / 1024)} GB`}
                                />
                            )}
                        </div>
                    </motion.div>

                    {/* Timeline Card */}
                    <motion.div
                        className="rounded-xl border border-white/10 bg-white/[0.02] p-6"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.35 }}
                    >
                        <h3 className="mb-4 font-display text-sm text-white">
                            Timeline
                        </h3>
                        <div className="space-y-4">
                            <InfoRow
                                label="Created"
                                value={formatDate(machine.created_at)}
                            />
                            <InfoRow
                                label="Last Seen"
                                value={formatDate(machine.last_seen_at)}
                            />
                            <InfoRow
                                label="Last Updated"
                                value={formatDate(machine.updated_at)}
                            />
                        </div>
                    </motion.div>
                </div>
            </div>

            {/* Modals */}
            <ConfirmModal
                open={showPauseConfirm}
                title="Pause Machine"
                description={
                    <>
                        Are you sure you want to pause{' '}
                        <span className="font-semibold text-white">
                            {machine.name}
                        </span>
                        ? The machine will not accept new jobs until resumed.
                    </>
                }
                confirmLabel="Pause"
                confirmColor="bg-amber-500 hover:bg-amber-400"
                icon={RiPauseLine}
                iconColor="text-amber-400"
                isLoading={pauseMachine.isPending}
                onConfirm={handleConfirmPause}
                onCancel={() => setShowPauseConfirm(false)}
            />

            <ConfirmModal
                open={showResumeConfirm}
                title="Resume Machine"
                description={
                    <>
                        Are you sure you want to resume{' '}
                        <span className="font-semibold text-white">
                            {machine.name}
                        </span>
                        ? The machine will be available for new jobs.
                    </>
                }
                confirmLabel="Resume"
                confirmColor="bg-emerald-500 hover:bg-emerald-400"
                icon={RiPlayCircleLine}
                iconColor="text-emerald-400"
                isLoading={resumeMachine.isPending}
                onConfirm={handleConfirmResume}
                onCancel={() => setShowResumeConfirm(false)}
            />

            <ConfirmModal
                open={showDeleteConfirm}
                title="Delete Machine"
                description={
                    <>
                        Are you sure you want to delete{' '}
                        <span className="font-semibold text-white">
                            {machine.name}
                        </span>
                        ? This action cannot be undone and will cancel all
                        associated runners.
                    </>
                }
                confirmLabel="Delete"
                confirmColor="bg-red-500 hover:bg-red-400"
                icon={RiAlertLine}
                iconColor="text-red-400"
                isLoading={deleteMachine.isPending}
                onConfirm={handleConfirmDelete}
                onCancel={() => setShowDeleteConfirm(false)}
            />

            <EditLimitsModal
                open={showEditLimits}
                machine={machine}
                cpuLimit={editCPULimit}
                ramLimit={editRAMLimit}
                error={limitsError}
                hasChanges={hasChanges || false}
                isLoading={updateMachineLimit.isPending}
                onCPUChange={setEditCPULimit}
                onRAMChange={setEditRAMLimit}
                onConfirm={handleConfirmEditLimits}
                onCancel={() => setShowEditLimits(false)}
            />
        </motion.div>
    );
}

function BackButton({ onClick }: { onClick: () => void }) {
    return (
        <button
            onClick={onClick}
            className="inline-flex items-center gap-2 font-mono text-sm text-white/60 transition-colors hover:text-white"
        >
            <RiArrowLeftLine className="size-4" />
            Back to Machines
        </button>
    );
}

function SettingsMenu({
    status,
    onEditLimits,
    onPause,
    onResume,
    onDelete,
}: {
    status: string;
    onEditLimits: () => void;
    onPause: () => void;
    onResume: () => void;
    onDelete: () => void;
}) {
    return (
        <div className="absolute right-0 z-10 mt-2 w-52 overflow-hidden rounded-lg border border-white/10 bg-[#0a0a0c] shadow-xl">
            <button
                onClick={onEditLimits}
                className="flex w-full items-center gap-3 px-4 py-3 text-left font-mono text-sm text-white/80 transition-colors hover:bg-white/5"
            >
                <RiEditLine className="size-4 text-white/50" />
                Edit Resource Limits
            </button>
            {status === 'online' && (
                <button
                    onClick={onPause}
                    className="flex w-full items-center gap-3 px-4 py-3 text-left font-mono text-sm text-white/80 transition-colors hover:bg-white/5"
                >
                    <RiPauseLine className="size-4 text-white/50" />
                    Pause Machine
                </button>
            )}
            {status === 'paused' && (
                <button
                    onClick={onResume}
                    className="flex w-full items-center gap-3 px-4 py-3 text-left font-mono text-sm text-white/80 transition-colors hover:bg-white/5"
                >
                    <RiPlayCircleLine className="size-4 text-white/50" />
                    Resume Machine
                </button>
            )}
            <div className="border-t border-white/10" />
            <button
                onClick={onDelete}
                className="flex w-full items-center gap-3 px-4 py-3 text-left font-mono text-sm text-red-400 transition-colors hover:bg-red-500/10"
            >
                <RiDeleteBin6Line className="size-4" />
                Delete Machine
            </button>
        </div>
    );
}

function RunnerCard({ runner, index }: { runner: Runner; index: number }) {
    const getStatusConfig = (status: string) => {
        switch (status) {
            case 'busy':
                return {
                    color: 'text-emerald-400',
                    bg: 'bg-emerald-500/10',
                    border: 'border-emerald-500/20',
                    label: 'Busy',
                };
            case 'idle':
                return {
                    color: 'text-blue-400',
                    bg: 'bg-blue-500/10',
                    border: 'border-blue-500/20',
                    label: 'Idle',
                };
            case 'pending':
                return {
                    color: 'text-amber-400',
                    bg: 'bg-amber-500/10',
                    border: 'border-amber-500/20',
                    label: 'Pending',
                };
            default:
                return {
                    color: 'text-white/40',
                    bg: 'bg-white/5',
                    border: 'border-white/10',
                    label: status,
                };
        }
    };

    const statusConfig = getStatusConfig(runner.status);

    return (
        <motion.div
            className="rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-colors hover:bg-white/[0.04]"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, delay: 0.4 + index * 0.05 }}
        >
            <div className="flex items-center justify-between gap-4">
                <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                        <p className="truncate font-mono text-sm text-white">
                            {runner.name}
                        </p>
                        <span
                            className={cx(
                                'inline-flex items-center rounded-full border px-2 py-0.5 text-xs capitalize',
                                statusConfig.bg,
                                statusConfig.border,
                                statusConfig.color,
                            )}
                        >
                            {statusConfig.label}
                        </span>
                    </div>
                    <div className="mt-2 flex items-center gap-3 font-mono text-xs text-white/40">
                        <span>{runner.arch}</span>
                        <span>
                            {runner.cpu} vCPU / {Math.round(runner.ram / 1024)} GB
                        </span>
                        {runner.labels && runner.labels.length > 0 && (
                            <span className="text-blue-400">
                                {runner.labels.slice(0, 2).join(', ')}
                                {runner.labels.length > 2 &&
                                    ` +${runner.labels.length - 2}`}
                            </span>
                        )}
                    </div>
                </div>
            </div>
        </motion.div>
    );
}

function InfoRow({ label, value }: { label: string; value: string }) {
    return (
        <div className="flex items-center justify-between">
            <span className="font-mono text-xs text-white/40">{label}</span>
            <span className="font-mono text-sm text-white/70">{value}</span>
        </div>
    );
}

function ConfirmModal({
    open,
    title,
    description,
    confirmLabel,
    confirmColor,
    icon: Icon,
    iconColor,
    isLoading,
    onConfirm,
    onCancel,
}: {
    open: boolean;
    title: string;
    description: React.ReactNode;
    confirmLabel: string;
    confirmColor: string;
    icon: typeof RiAlertLine;
    iconColor: string;
    isLoading: boolean;
    onConfirm: () => void;
    onCancel: () => void;
}) {
    if (!open) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
            <motion.div
                className="w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ duration: 0.2 }}
            >
                <div className="mb-6 flex items-start gap-4">
                    <div
                        className={cx(
                            'flex size-10 items-center justify-center rounded-full',
                            iconColor.replace('text-', 'bg-').replace('400', '500/20'),
                        )}
                    >
                        <Icon className={cx('size-5', iconColor)} />
                    </div>
                    <div className="flex-1">
                        <h3 className="font-display text-lg text-white">{title}</h3>
                        <p className="mt-2 font-mono text-sm text-white/60">
                            {description}
                        </p>
                    </div>
                </div>
                <div className="flex justify-end gap-3">
                    <button
                        onClick={onCancel}
                        disabled={isLoading}
                        className="rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:bg-white/5 disabled:opacity-50"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={onConfirm}
                        disabled={isLoading}
                        className={cx(
                            'rounded-lg px-4 py-2 font-mono text-sm text-white transition-colors disabled:opacity-50',
                            confirmColor,
                        )}
                    >
                        {isLoading ? 'Processing...' : confirmLabel}
                    </button>
                </div>
            </motion.div>
        </div>
    );
}

function EditLimitsModal({
    open,
    machine,
    cpuLimit,
    ramLimit,
    error,
    hasChanges,
    isLoading,
    onCPUChange,
    onRAMChange,
    onConfirm,
    onCancel,
}: {
    open: boolean;
    machine: { cpu: number; ram_total: number };
    cpuLimit: string;
    ramLimit: string;
    error: string;
    hasChanges: boolean;
    isLoading: boolean;
    onCPUChange: (value: string) => void;
    onRAMChange: (value: string) => void;
    onConfirm: () => void;
    onCancel: () => void;
}) {
    if (!open) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
            <motion.div
                className="w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ duration: 0.2 }}
            >
                <h3 className="mb-4 font-display text-lg text-white">
                    Edit Resource Limits
                </h3>

                <div className="mb-6 rounded-lg border border-blue-500/20 bg-blue-500/10 p-4">
                    <p className="mb-2 font-mono text-sm text-blue-400">
                        Discovered Resources
                    </p>
                    <div className="space-y-1 font-mono text-sm text-blue-300/80">
                        {machine.cpu > 0 ? (
                            <p>CPU: {machine.cpu} vCPU</p>
                        ) : (
                            <p className="text-blue-400/60">
                                CPU: Not yet discovered
                            </p>
                        )}
                        {machine.ram_total > 0 ? (
                            <p>RAM: {Math.round(machine.ram_total / 1024)} GB</p>
                        ) : (
                            <p className="text-blue-400/60">
                                RAM: Not yet discovered
                            </p>
                        )}
                    </div>
                </div>

                {error && (
                    <div className="mb-4 rounded-lg border border-red-500/20 bg-red-500/10 p-3">
                        <p className="font-mono text-sm text-red-400">{error}</p>
                    </div>
                )}

                <div className="mb-6 space-y-4">
                    <div>
                        <label className="mb-2 block font-mono text-xs text-white/60">
                            CPU Limit (vCPU)
                        </label>
                        <input
                            type="number"
                            min="0"
                            value={cpuLimit}
                            onChange={(e) => onCPUChange(e.target.value)}
                            placeholder="Leave empty for unlimited"
                            className="w-full rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2.5 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                        />
                    </div>

                    <div>
                        <label className="mb-2 block font-mono text-xs text-white/60">
                            RAM Limit (GB)
                        </label>
                        <input
                            type="number"
                            min="0"
                            value={ramLimit}
                            onChange={(e) => onRAMChange(e.target.value)}
                            placeholder="Leave empty for unlimited"
                            className="w-full rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2.5 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                        />
                    </div>
                </div>

                <div className="flex justify-end gap-3">
                    <button
                        onClick={onCancel}
                        disabled={isLoading}
                        className="rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:bg-white/5 disabled:opacity-50"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={onConfirm}
                        disabled={!hasChanges || isLoading}
                        className="rounded-lg bg-white px-4 py-2 font-mono text-sm text-black transition-colors hover:bg-white/90 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                        {isLoading ? 'Saving...' : 'Save Limits'}
                    </button>
                </div>
            </motion.div>
        </div>
    );
}
