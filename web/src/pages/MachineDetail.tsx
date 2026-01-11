import { useInstallation } from '@/hooks/useInstallation';
import { useMachineDetail } from '@/hooks/useMachineDetail';
import { useMachineMutations } from '@/hooks/useMachineMutations';
import { MembershipRoleAdmin } from '@/types/installation';
import {
    RiAlertLine,
    RiArrowLeftLine,
    RiCpuLine,
    RiDeleteBin6Line,
    RiEditLine,
    RiMoreLine,
    RiPauseCircleLine,
    RiPlayCircleLine,
    RiRam2Line,
    RiServerLine,
} from '@remixicon/react';
import { useNavigate, useParams } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useEffect, useRef, useState } from 'react';
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

    const isAdmin =
        selectedInstallation?.membership?.role === MembershipRoleAdmin;

    // Use login from params or selectedInstallation as fallback
    const currentLogin = login || selectedInstallation?.login;

    // Helper function to get status dot styling
    const getStatusDotColor = (status: string) => {
        switch (status) {
            case 'online':
                return 'bg-emerald-500 shadow-lg shadow-emerald-500/50 animate-pulse';
            case 'offline':
                return 'bg-white/30';
            case 'paused':
                return 'bg-amber-500';
            default:
                return 'bg-white/30';
        }
    };

    // Helper function to format last seen date
    const formatLastSeenDate = (timestamp: number) => {
        // If timestamp is 0 or very close to epoch (1970), display "never"
        if (timestamp === 0) {
            return 'Never';
        }
        return new Date(timestamp * 1000).toLocaleString('en-US', {
            year: 'numeric',
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
        });
    };

    // Handler functions for settings
    const handlePauseMachine = () => {
        setShowPauseConfirm(true);
        setShowSettings(false);
    };

    const handleConfirmPause = async () => {
        try {
            await pauseMachine.mutateAsync(machineName);
            setShowPauseConfirm(false);
        } catch (error) {
            console.error('Failed to pause machine:', error);
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
        } catch (error) {
            console.error('Failed to resume machine:', error);
        }
    };

    const handleDeleteMachineClick = () => {
        setShowDeleteConfirm(true);
        setShowSettings(false);
    };

    const handleConfirmDelete = async () => {
        const deletePromise = async () => {
            await deleteMachine.mutateAsync(machineName);
            setShowDeleteConfirm(false);
            // Navigate back to machines list after successful deletion
            navigate({
                to: '/$login/machines',
                params: { login: currentLogin || 'org' },
            });
            return { success: true, name: machineName };
        };

        toast.promise(deletePromise(), {
            loading: `Deleting machine "${machineName}"...`,
            success: (data) => `Machine "${data.name}" deleted successfully`,
            error: 'Failed to delete machine. Please try again.',
        });
    };

    const handleEditLimitsClick = () => {
        if (machine) {
            const cpuLimitStr =
                machine.cpu_limit > 0 ? machine.cpu_limit.toString() : '';
            const ramLimitStr =
                machine.ram_limit > 0
                    ? Math.round(machine.ram_limit / 1024).toString()
                    : '';
            setEditCPULimit(cpuLimitStr);
            setEditRAMLimit(ramLimitStr);
            setLimitsError('');
            setShowEditLimits(true);
            setShowSettings(false);
        }
    };

    // Check if there are any changes from the original limits
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

        // Validate that inputs are valid numbers if provided
        if (editCPULimit && isNaN(cpuLimit)) {
            setLimitsError('Invalid CPU limit');
            return;
        }

        if (editRAMLimit && isNaN(ramLimitGB)) {
            setLimitsError('Invalid RAM limit');
            return;
        }

        // Validate against discovered resources
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

    // Calculate resource usage from machine
    const machineRunners = machine?.runners ?? [];

    // Determine effective limits (use total if limit is 0, which means "unknown")
    const cpuLimit = machine
        ? machine.cpu_limit > 0
            ? machine.cpu_limit
            : machine.cpu
        : 0;
    const ramLimit = machine
        ? machine.ram_limit > 0
            ? machine.ram_limit
            : machine.ram_available
        : 0;

    const cpuAllocated = machine?.cpu_allocated || 0;
    const ramAllocated = machine?.ram_allocated || 0;
    const cpuUsagePercent =
        cpuLimit > 0 ? Math.round((cpuAllocated / cpuLimit) * 100) : 0;
    const ramUsagePercent =
        ramLimit > 0 ? Math.round((ramAllocated / ramLimit) * 100) : 0;

    if (isLoading) {
        return (
            <div className="space-y-8">
                <button
                    onClick={() =>
                        navigate({
                            to: '/$login/machines',
                            params: { login: currentLogin || 'org' },
                        })
                    }
                    className="flex items-center gap-2 font-mono text-sm text-amber-400 transition-colors hover:text-amber-300"
                >
                    <RiArrowLeftLine className="size-4" aria-hidden="true" />
                    Back to Machines
                </button>
                <div className="h-12 w-48 animate-pulse rounded-lg bg-white/[0.02]" />
                <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
                    <div className="lg:col-span-2">
                        <div className="h-64 w-full animate-pulse rounded-xl bg-white/[0.02]" />
                    </div>
                    <div>
                        <div className="h-64 w-full animate-pulse rounded-xl bg-white/[0.02]" />
                    </div>
                </div>
            </div>
        );
    }

    if (error || !machine) {
        return (
            <div className="space-y-4">
                <button
                    onClick={() =>
                        navigate({
                            to: '/$login/machines',
                            params: { login: currentLogin || 'org' },
                        })
                    }
                    className="flex items-center gap-2 font-mono text-sm text-amber-400 transition-colors hover:text-amber-300"
                >
                    <RiArrowLeftLine className="size-4" aria-hidden="true" />
                    Back to Machines
                </button>
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

    return (
        <motion.div
            className="space-y-8"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
        >
            {/* Back Button */}
            <button
                onClick={() =>
                    navigate({
                        to: '/$login/machines',
                        params: { login: currentLogin || 'org' },
                    })
                }
                className="flex items-center gap-2 font-mono text-sm text-amber-400 transition-colors hover:text-amber-300"
            >
                <RiArrowLeftLine className="size-4" aria-hidden="true" />
                Back to Machines
            </button>

            {/* Header */}
            <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                    <div className="flex min-w-0 items-center gap-3">
                        <h1 className="truncate font-display text-3xl text-white">
                            {machine.name}
                        </h1>
                        {/* Status dot */}
                        <div
                            className={`h-3 w-3 flex-shrink-0 rounded-full ${getStatusDotColor(machine.status)}`}
                        />
                    </div>
                    <p className="mt-1 font-mono text-sm text-white/50">
                        Last seen: {formatLastSeenDate(machine.last_seen_at)}
                    </p>
                </div>
                {isAdmin && (
                    <div className="relative flex-shrink-0" ref={settingsRef}>
                        <button
                            onClick={() => setShowSettings(!showSettings)}
                            className="rounded-lg p-2 transition-colors hover:bg-white/5"
                            title="Machine settings"
                        >
                            <RiMoreLine
                                className="size-5 text-white/60"
                                aria-hidden="true"
                            />
                        </button>
                        {showSettings && (
                            <div className="absolute right-0 z-10 mt-2 w-48 rounded-lg border border-white/10 bg-[#0a0a0c] shadow-lg">
                                <button
                                    onClick={handleEditLimitsClick}
                                    className="flex w-full items-center gap-3 border-b border-white/10 px-4 py-2 text-left font-mono text-sm text-white/80 transition-colors hover:bg-white/5"
                                >
                                    <RiEditLine
                                        className="size-4 text-white/50"
                                        aria-hidden="true"
                                    />
                                    Edit Resource Limits
                                </button>
                                {machine.status === 'online' && (
                                    <button
                                        onClick={handlePauseMachine}
                                        className="flex w-full items-center gap-3 border-b border-white/10 px-4 py-2 text-left font-mono text-sm text-white/80 transition-colors hover:bg-white/5"
                                    >
                                        <RiPauseCircleLine
                                            className="size-4 text-white/50"
                                            aria-hidden="true"
                                        />
                                        Pause Machine
                                    </button>
                                )}
                                {machine.status === 'paused' && (
                                    <button
                                        onClick={handleResumeMachine}
                                        className="flex w-full items-center gap-3 border-b border-white/10 px-4 py-2 text-left font-mono text-sm text-white/80 transition-colors hover:bg-white/5"
                                    >
                                        <RiPlayCircleLine
                                            className="size-4 text-white/50"
                                            aria-hidden="true"
                                        />
                                        Resume Machine
                                    </button>
                                )}
                                <button
                                    onClick={handleDeleteMachineClick}
                                    className="flex w-full items-center gap-3 px-4 py-2 text-left font-mono text-sm text-red-400 transition-colors hover:bg-red-500/10"
                                >
                                    <RiDeleteBin6Line
                                        className="size-4 text-red-400"
                                        aria-hidden="true"
                                    />
                                    Delete Machine
                                </button>
                            </div>
                        )}
                    </div>
                )}
            </div>

            {/* Main Grid */}
            <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
                {/* Left Column - Primary Info */}
                <div className="space-y-6 lg:col-span-2">
                    {/* Machine Details - Combined Card */}
                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                        {/* Labels */}
                        {machine.labels && machine.labels.length > 0 && (
                            <div className="mb-6 border-b border-white/10 pb-6">
                                <p className="mb-3 font-mono text-xs text-white/50">
                                    Labels
                                </p>
                                <div className="flex flex-wrap gap-2">
                                    {machine.labels.map((label) => (
                                        <span
                                            key={label}
                                            className="inline-flex items-center rounded-full bg-white/5 px-3 py-1 font-mono text-xs text-white/70"
                                        >
                                            {label}
                                        </span>
                                    ))}
                                </div>
                            </div>
                        )}

                        {/* Resources */}
                        <div>
                            <h2 className="mb-4 font-display text-sm text-white">
                                Resources
                            </h2>
                            <div className="space-y-6">
                                {/* CPU */}
                                <div>
                                    <div className="mb-3 flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <RiCpuLine
                                                className="size-5 text-purple-400"
                                                aria-hidden="true"
                                            />
                                            <span className="font-mono text-xs text-white/50">
                                                CPU
                                            </span>
                                        </div>
                                        <div className="text-right">
                                            <p className="font-display text-lg text-white">
                                                {machine.cpu > 0
                                                    ? `${cpuUsagePercent}%`
                                                    : 'Unknown'}
                                            </p>
                                            <p className="font-mono text-xs text-white/40">
                                                {machine.cpu > 0
                                                    ? `${cpuAllocated} / ${cpuLimit} vCPU`
                                                    : 'Data unavailable'}
                                            </p>
                                        </div>
                                    </div>
                                    {machine.cpu > 0 && (
                                        <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
                                            <div
                                                className="h-full bg-purple-500 transition-all"
                                                style={{
                                                    width: `${cpuUsagePercent}%`,
                                                }}
                                            />
                                        </div>
                                    )}
                                    {machine.cpu > 0 && (
                                        <div className="mt-3 space-y-1 font-mono text-xs text-white/40">
                                            <div className="flex justify-between">
                                                <span>Allocated:</span>
                                                <span className="text-white/60">
                                                    {cpuAllocated} vCPU
                                                </span>
                                            </div>
                                            <div className="flex justify-between">
                                                <span>Available:</span>
                                                <span className="text-white/60">
                                                    {cpuLimit - cpuAllocated}{' '}
                                                    vCPU
                                                </span>
                                            </div>
                                            {machine.cpu_limit > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Limit:</span>
                                                    <span className="text-white/60">
                                                        {cpuLimit} vCPU
                                                    </span>
                                                </div>
                                            )}
                                            {machine.cpu > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Total:</span>
                                                    <span className="text-white/60">
                                                        {machine.cpu} vCPU
                                                    </span>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>

                                {/* RAM */}
                                <div>
                                    <div className="mb-3 flex items-center justify-between">
                                        <div className="flex items-center gap-2">
                                            <RiRam2Line
                                                className="size-5 text-orange-400"
                                                aria-hidden="true"
                                            />
                                            <span className="font-mono text-xs text-white/50">
                                                RAM
                                            </span>
                                        </div>
                                        <div className="text-right">
                                            <p className="font-display text-lg text-white">
                                                {machine.ram_available > 0
                                                    ? `${ramUsagePercent}%`
                                                    : 'Unknown'}
                                            </p>
                                            <p className="font-mono text-xs text-white/40">
                                                {machine.ram_available > 0
                                                    ? `${Math.round(ramAllocated / 1024)} / ${Math.round(ramLimit / 1024)} GB`
                                                    : 'Data unavailable'}
                                            </p>
                                        </div>
                                    </div>
                                    {machine.ram_available > 0 && (
                                        <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
                                            <div
                                                className="h-full bg-orange-500 transition-all"
                                                style={{
                                                    width: `${ramUsagePercent}%`,
                                                }}
                                            />
                                        </div>
                                    )}
                                    {machine.ram_available > 0 && (
                                        <div className="mt-3 space-y-1 font-mono text-xs text-white/40">
                                            <div className="flex justify-between">
                                                <span>Allocated:</span>
                                                <span className="text-white/60">
                                                    {Math.round(
                                                        ramAllocated / 1024,
                                                    )}{' '}
                                                    GB
                                                </span>
                                            </div>
                                            <div className="flex justify-between">
                                                <span>Available:</span>
                                                <span className="text-white/60">
                                                    {Math.round(
                                                        (ramLimit -
                                                            ramAllocated) /
                                                            1024,
                                                    )}{' '}
                                                    GB
                                                </span>
                                            </div>
                                            {machine.ram_limit > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Limit:</span>
                                                    <span className="text-white/60">
                                                        {Math.round(
                                                            ramLimit / 1024,
                                                        )}{' '}
                                                        GB
                                                    </span>
                                                </div>
                                            )}
                                            {machine.ram_total > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Total:</span>
                                                    <span className="text-white/60">
                                                        {Math.round(
                                                            machine.ram_total /
                                                                1024,
                                                        )}{' '}
                                                        GB
                                                    </span>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* Runners on this Machine */}
                    <div>
                        <h2 className="mb-4 font-display text-lg text-white">
                            Runners on this Machine
                        </h2>
                        {machineRunners.length === 0 ? (
                            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center">
                                <RiServerLine
                                    className="mx-auto mb-4 size-12 text-white/20"
                                    aria-hidden="true"
                                />
                                <p className="mb-2 font-display text-lg text-white">
                                    No runners yet
                                </p>
                                <p className="font-mono text-xs text-white/50">
                                    No runners have been assigned to this
                                    machine.
                                </p>
                            </div>
                        ) : (
                            <div className="space-y-3">
                                {machineRunners.map((runner) => (
                                    <div
                                        key={runner.id}
                                        className="rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-colors hover:bg-white/[0.04]"
                                    >
                                        <div className="flex items-center justify-between gap-4">
                                            <div className="min-w-0 flex-1">
                                                <p className="font-mono text-sm text-white">
                                                    {runner.name}
                                                </p>
                                                <div className="mt-2 flex items-center gap-3">
                                                    <span className="inline-flex items-center rounded border border-blue-500/20 bg-blue-500/10 px-2.5 py-0.5 font-mono text-xs capitalize text-blue-400">
                                                        {runner.status}
                                                    </span>
                                                    <p className="font-mono text-xs text-white/40">
                                                        {runner.arch}
                                                    </p>
                                                    <p className="font-mono text-xs text-white/40">
                                                        {runner.cpu} vCPU •{' '}
                                                        {Math.round(
                                                            runner.ram / 1024,
                                                        )}{' '}
                                                        GB RAM
                                                    </p>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                </div>

                {/* Right Column - Architecture & Dates */}
                <div className="space-y-6">
                    {/* Architecture Card */}
                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                        <h2 className="mb-4 font-display text-sm text-white">
                            Architecture
                        </h2>
                        <div className="flex items-center gap-3">
                            <RiServerLine
                                className="size-5 text-blue-400"
                                aria-hidden="true"
                            />
                            <span className="font-mono text-sm capitalize text-white/70">
                                {machine.arch}
                            </span>
                        </div>
                    </div>

                    {/* Dates Card */}
                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                        <h2 className="mb-4 font-display text-sm text-white">
                            Dates
                        </h2>
                        <div className="space-y-3">
                            <div>
                                <p className="mb-1 font-mono text-xs text-white/40">
                                    Created
                                </p>
                                <p className="font-mono text-sm text-white/70">
                                    {new Date(
                                        machine.created_at * 1000,
                                    ).toLocaleString('en-US', {
                                        year: 'numeric',
                                        month: 'short',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    })}
                                </p>
                            </div>
                            <div>
                                <p className="mb-1 font-mono text-xs text-white/40">
                                    Last Seen
                                </p>
                                <p className="font-mono text-sm text-white/70">
                                    {formatLastSeenDate(machine.last_seen_at)}
                                </p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Pause Confirmation Modal */}
            {showPauseConfirm && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
                    <div className="mx-4 w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6">
                        <div className="mb-6 flex items-start gap-4">
                            <div className="flex-shrink-0">
                                <RiAlertLine
                                    className="size-6 text-amber-400"
                                    aria-hidden="true"
                                />
                            </div>
                            <div className="flex-1">
                                <h3 className="font-display text-lg text-white">
                                    Pause Machine
                                </h3>
                                <p className="mt-2 font-mono text-sm text-white/60">
                                    Are you sure you want to pause{' '}
                                    <span className="font-semibold text-white">
                                        {machine?.name}
                                    </span>
                                    ? The machine will not accept new jobs until
                                    resumed.
                                </p>
                            </div>
                        </div>
                        <div className="flex justify-end gap-3">
                            <button
                                onClick={() => setShowPauseConfirm(false)}
                                disabled={pauseMachine.isPending}
                                className="rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:opacity-50"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleConfirmPause}
                                disabled={pauseMachine.isPending}
                                className="rounded-md bg-amber-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400 disabled:opacity-50"
                            >
                                {pauseMachine.isPending
                                    ? 'Pausing...'
                                    : 'Pause'}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Resume Confirmation Modal */}
            {showResumeConfirm && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
                    <div className="mx-4 w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6">
                        <div className="mb-6 flex items-start gap-4">
                            <div className="flex-shrink-0">
                                <RiAlertLine
                                    className="size-6 text-emerald-400"
                                    aria-hidden="true"
                                />
                            </div>
                            <div className="flex-1">
                                <h3 className="font-display text-lg text-white">
                                    Resume Machine
                                </h3>
                                <p className="mt-2 font-mono text-sm text-white/60">
                                    Are you sure you want to resume{' '}
                                    <span className="font-semibold text-white">
                                        {machine?.name}
                                    </span>
                                    ? The machine will be available for new
                                    jobs.
                                </p>
                            </div>
                        </div>
                        <div className="flex justify-end gap-3">
                            <button
                                onClick={() => setShowResumeConfirm(false)}
                                disabled={resumeMachine.isPending}
                                className="rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:opacity-50"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleConfirmResume}
                                disabled={resumeMachine.isPending}
                                className="rounded-md bg-emerald-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-emerald-400 disabled:opacity-50"
                            >
                                {resumeMachine.isPending
                                    ? 'Resuming...'
                                    : 'Resume'}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Delete Confirmation Modal */}
            {showDeleteConfirm && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
                    <div className="mx-4 w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6">
                        <div className="mb-6 flex items-start gap-4">
                            <div className="flex-shrink-0">
                                <RiAlertLine
                                    className="size-6 text-red-400"
                                    aria-hidden="true"
                                />
                            </div>
                            <div className="flex-1">
                                <h3 className="font-display text-lg text-white">
                                    Delete Machine
                                </h3>
                                <p className="mt-2 font-mono text-sm text-white/60">
                                    Are you sure you want to delete{' '}
                                    <span className="font-semibold text-white">
                                        {machine?.name}
                                    </span>
                                    ? This action cannot be undone.
                                </p>
                            </div>
                        </div>
                        <div className="flex justify-end gap-3">
                            <button
                                onClick={() => setShowDeleteConfirm(false)}
                                disabled={deleteMachine.isPending}
                                className="rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:opacity-50"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleConfirmDelete}
                                disabled={deleteMachine.isPending}
                                className="rounded-md bg-red-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-red-400 disabled:opacity-50"
                            >
                                {deleteMachine.isPending
                                    ? 'Deleting...'
                                    : 'Delete'}
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Edit Resource Limits Modal */}
            {showEditLimits && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
                    <div className="mx-4 w-full max-w-md rounded-xl border border-white/10 bg-[#0a0a0c] p-6">
                        <h3 className="mb-4 font-display text-lg text-white">
                            Edit Resource Limits
                        </h3>

                        {/* Current Resources Info */}
                        {machine && (
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
                                        <p>
                                            RAM:{' '}
                                            {Math.round(
                                                machine.ram_total / 1024,
                                            )}{' '}
                                            GB
                                        </p>
                                    ) : (
                                        <p className="text-blue-400/60">
                                            RAM: Not yet discovered
                                        </p>
                                    )}
                                </div>
                            </div>
                        )}

                        {/* Error Message */}
                        {limitsError && (
                            <div className="mb-4 rounded-lg border border-red-500/20 bg-red-500/10 p-3">
                                <p className="font-mono text-sm text-red-400">
                                    {limitsError}
                                </p>
                            </div>
                        )}

                        {/* Input Fields */}
                        <div className="mb-6 space-y-4">
                            <div>
                                <label className="mb-1 block font-mono text-xs text-white/60">
                                    CPU Limit (vCPU)
                                </label>
                                <input
                                    type="number"
                                    min="0"
                                    value={editCPULimit}
                                    onChange={(e) =>
                                        setEditCPULimit(e.target.value)
                                    }
                                    placeholder="Leave empty for unlimited"
                                    className="w-full rounded-md border border-white/10 bg-white/[0.02] px-3 py-2 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                                />
                                <p className="mt-1 font-mono text-[11px] text-white/30">
                                    Leave empty to unset the limit
                                </p>
                            </div>

                            <div>
                                <label className="mb-1 block font-mono text-xs text-white/60">
                                    RAM Limit (GB)
                                </label>
                                <input
                                    type="number"
                                    min="0"
                                    value={editRAMLimit}
                                    onChange={(e) =>
                                        setEditRAMLimit(e.target.value)
                                    }
                                    placeholder="Leave empty for unlimited"
                                    className="w-full rounded-md border border-white/10 bg-white/[0.02] px-3 py-2 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                                />
                                <p className="mt-1 font-mono text-[11px] text-white/30">
                                    Leave empty to unset the limit
                                </p>
                            </div>
                        </div>

                        {/* Buttons */}
                        <div className="flex justify-end gap-3">
                            <button
                                onClick={() => setShowEditLimits(false)}
                                disabled={updateMachineLimit.isPending}
                                className="rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:opacity-50"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleConfirmEditLimits}
                                disabled={
                                    !hasChanges || updateMachineLimit.isPending
                                }
                                className="rounded-md bg-amber-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400 disabled:cursor-not-allowed disabled:opacity-50"
                            >
                                {updateMachineLimit.isPending
                                    ? 'Saving...'
                                    : 'Save Limits'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </motion.div>
    );
}
