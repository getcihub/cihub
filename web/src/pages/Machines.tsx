import { AddMachineModal } from '@/components/AddMachineModal';
import { useInstallation } from '@/hooks/useInstallation';
import { useMachines } from '@/hooks/useMachines';
import { MembershipRoleAdmin } from '@/types/installation';
import { MachineStatusOnline } from '@/types/machine';
import {
    RiAddLine,
    RiArrowRightSLine,
    RiCpuLine,
    RiRam2Line,
    RiServerLine,
} from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useState } from 'react';

export function MachinesPage() {
    const navigate = useNavigate();
    const { selectedInstallation } = useInstallation();
    const { data: machines = [], isLoading, error } = useMachines();
    const [selectedStatus, setSelectedStatus] = useState<string>('all');
    const [isAddMachineModalOpen, setIsAddMachineModalOpen] = useState(false);

    const isAdmin =
        selectedInstallation?.membership?.role === MembershipRoleAdmin;

    // Helper function to get runner count from machine
    const getMachineRunnerCount = (machine: { runners?: unknown[] }) => {
        const machineRunners = machine.runners ?? [];
        return { runnerCount: machineRunners.length };
    };

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

    // Calculate statistics
    const totalMachines = machines.length;
    const onlineMachines = machines.filter(
        (m) => m.status === MachineStatusOnline,
    ).length;
    const totalCPUAllocated = machines.reduce(
        (sum, m) => sum + m.cpu_allocated,
        0,
    );
    const totalRAMAllocated = machines.reduce(
        (sum, m) => sum + m.ram_allocated,
        0,
    );

    // Calculate total CPU limit based on machine limits
    const totalCPULimit = machines.reduce((sum, m) => {
        if (m.cpu_limit > 0) {
            return sum + m.cpu_limit;
        }
        return sum + m.cpu;
    }, 0);

    // Calculate total RAM limit based on machine limits
    const totalRAMLimit = machines.reduce((sum, m) => {
        if (m.ram_limit > 0) {
            return sum + m.ram_limit;
        }
        return sum + m.ram_total;
    }, 0);

    const cpuUsagePercent =
        totalCPULimit > 0
            ? Math.round((totalCPUAllocated / totalCPULimit) * 100)
            : 0;
    const ramUsagePercent =
        totalRAMLimit > 0
            ? Math.round((totalRAMAllocated / totalRAMLimit) * 100)
            : 0;
    const ramUsageGB = Math.round(totalRAMAllocated / 1024);
    const totalRAMGB = Math.round(totalRAMLimit / 1024);

    const handleMachineClick = (machineName: string) => {
        navigate({
            to: '/$login/machines/$name',
            params: { login: selectedInstallation!.login, name: machineName },
        });
    };

    // Helper function to count machines by status
    const getStatusCount = (status: string) => {
        if (status === 'all') {
            return machines.length;
        }
        return machines.filter((m) => m.status === status).length;
    };

    // Filter machines based on selected status
    const filteredMachines =
        selectedStatus === 'all'
            ? machines
            : machines.filter((m) => m.status === selectedStatus);

    if (isLoading) {
        return (
            <div className="space-y-8">
                <div>
                    <h1 className="font-display text-3xl text-white">
                        Machines
                    </h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Manage your self-hosted runners
                    </p>
                </div>
                <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                    {[...Array(3)].map((_, i) => (
                        <div
                            key={i}
                            className="h-32 animate-pulse rounded-xl bg-white/[0.02] ring-1 ring-white/5"
                        />
                    ))}
                </div>
                <div className="space-y-4">
                    {[...Array(5)].map((_, i) => (
                        <div
                            key={i}
                            className="h-20 animate-pulse rounded-xl bg-white/[0.02] ring-1 ring-white/5"
                        />
                    ))}
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="space-y-4">
                <h1 className="font-display text-3xl text-white">Machines</h1>
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-6">
                    <p className="font-mono text-sm text-red-400">
                        Failed to load machines. Please try again later.
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
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="font-display text-3xl text-white">
                        Machines
                    </h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Manage your self-hosted runners
                    </p>
                </div>
                {isAdmin && (
                    <button
                        onClick={() => setIsAddMachineModalOpen(true)}
                        className="inline-flex items-center gap-2 rounded-md bg-amber-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400"
                    >
                        <RiAddLine className="size-4" />
                        Add Machine
                    </button>
                )}
            </div>

            {/* Filter Bar */}
            <div className="flex gap-2 overflow-x-auto pb-2">
                {['all', 'online', 'offline', 'paused'].map((status) => (
                    <button
                        key={status}
                        onClick={() => setSelectedStatus(status)}
                        className={`whitespace-nowrap rounded-lg px-4 py-2 font-mono text-sm transition-all ${
                            selectedStatus === status
                                ? 'bg-amber-500 text-white'
                                : 'bg-white/[0.02] text-white/70 hover:bg-white/5 hover:text-white'
                        }`}
                    >
                        <span className="capitalize">
                            {status === 'all' ? 'All Machines' : status}
                        </span>
                        <span className="ml-2 text-xs opacity-75">
                            ({getStatusCount(status)})
                        </span>
                    </button>
                ))}
            </div>

            {/* Stats Cards */}
            <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="font-mono text-xs text-white/50">
                                Total Machines
                            </p>
                            <div className="mt-2">
                                <p className="font-display text-3xl text-white">
                                    {totalMachines}
                                </p>
                                <p className="mt-1 font-mono text-xs text-white/40">
                                    {onlineMachines} online
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-blue-500/20 p-3">
                            <RiServerLine
                                className="size-6 text-blue-400"
                                aria-hidden="true"
                            />
                        </div>
                    </div>
                </div>

                <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="font-mono text-xs text-white/50">
                                CPU Usage
                            </p>
                            <div className="mt-2">
                                <p className="font-display text-3xl text-white">
                                    {cpuUsagePercent}%
                                </p>
                                <p className="mt-1 font-mono text-xs text-white/40">
                                    {totalCPUAllocated} / {totalCPULimit} vCPU
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-purple-500/20 p-3">
                            <RiCpuLine
                                className="size-6 text-purple-400"
                                aria-hidden="true"
                            />
                        </div>
                    </div>
                    <div className="mt-4 h-1.5 overflow-hidden rounded-full bg-white/10">
                        <div
                            className="h-full bg-purple-500 transition-all"
                            style={{ width: `${cpuUsagePercent}%` }}
                        />
                    </div>
                </div>

                <div className="rounded-xl border border-white/10 bg-white/[0.02] p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="font-mono text-xs text-white/50">
                                RAM Usage
                            </p>
                            <div className="mt-2">
                                <p className="font-display text-3xl text-white">
                                    {ramUsagePercent}%
                                </p>
                                <p className="mt-1 font-mono text-xs text-white/40">
                                    {ramUsageGB} / {totalRAMGB} GB
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-orange-500/20 p-3">
                            <RiRam2Line
                                className="size-6 text-orange-400"
                                aria-hidden="true"
                            />
                        </div>
                    </div>
                    <div className="mt-4 h-1.5 overflow-hidden rounded-full bg-white/10">
                        <div
                            className="h-full bg-orange-500 transition-all"
                            style={{ width: `${ramUsagePercent}%` }}
                        />
                    </div>
                </div>
            </div>

            {/* Machines List */}
            {filteredMachines.length === 0 ? (
                <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8 text-center">
                    <RiServerLine
                        className="mx-auto mb-4 size-12 text-white/20"
                        aria-hidden="true"
                    />
                    <p className="mb-2 font-display text-lg text-white">
                        {machines.length === 0
                            ? 'No machines yet'
                            : `No ${selectedStatus} machines`}
                    </p>
                    <p className="mb-6 font-mono text-xs text-white/50">
                        {machines.length === 0
                            ? 'No machines have been registered. Install the CIHub agent to get started.'
                            : `No machines with status "${selectedStatus}" found.`}
                    </p>
                    {isAdmin && machines.length === 0 && (
                        <button
                            onClick={() => setIsAddMachineModalOpen(true)}
                            className="inline-flex items-center gap-2 rounded-md bg-amber-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400"
                        >
                            <RiAddLine className="size-4" />
                            Create Machine
                        </button>
                    )}
                </div>
            ) : (
                <div className="space-y-3">
                    {filteredMachines.map((machine, index) => {
                        const { runnerCount } = getMachineRunnerCount(machine);

                        // Determine effective limits (use total if limit is 0, which means "unknown")
                        const cpuLimit =
                            machine.cpu_limit > 0
                                ? machine.cpu_limit
                                : machine.cpu;
                        const ramLimit =
                            machine.ram_limit > 0
                                ? machine.ram_limit
                                : machine.ram_available;

                        const cpuPercent =
                            cpuLimit > 0
                                ? Math.round(
                                      (machine.cpu_allocated / cpuLimit) * 100,
                                  )
                                : 0;
                        const ramPercent =
                            ramLimit > 0
                                ? Math.round(
                                      (machine.ram_allocated / ramLimit) * 100,
                                  )
                                : 0;

                        return (
                            <motion.button
                                key={machine.name}
                                onClick={() => handleMachineClick(machine.name)}
                                className="group w-full text-left"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{
                                    duration: 0.3,
                                    delay: index * 0.05,
                                }}
                            >
                                <div className="flex items-stretch justify-between gap-4 rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-all hover:border-white/20 hover:bg-white/[0.04]">
                                    {/* Left side: Machine info */}
                                    <div className="flex min-w-0 flex-1 gap-3">
                                        {/* Status dot */}
                                        <div
                                            className={`mt-1.5 h-2 w-2 flex-shrink-0 rounded-full ${getStatusDotColor(machine.status)}`}
                                        />

                                        {/* Machine name and details */}
                                        <div className="min-w-0 flex-1">
                                            {/* Machine name */}
                                            <h3 className="truncate font-mono text-sm text-white">
                                                {machine.name}
                                            </h3>

                                            {/* Architecture, runners count, and labels */}
                                            <div className="mt-1 flex flex-wrap items-center gap-1">
                                                <span className="rounded bg-white/5 px-1.5 py-0.5 font-mono text-xs text-white/50">
                                                    {machine.arch}
                                                </span>
                                                <span className="text-xs text-white/30">
                                                    •
                                                </span>
                                                <span className="inline-flex items-center gap-1 rounded border border-blue-500/20 bg-blue-500/10 px-2 py-0.5 font-mono text-xs text-blue-400">
                                                    {runnerCount} runner
                                                    {runnerCount !== 1
                                                        ? 's'
                                                        : ''}
                                                </span>
                                                {machine.labels &&
                                                    machine.labels.length >
                                                        0 && (
                                                        <>
                                                            <span className="text-xs text-white/30">
                                                                •
                                                            </span>
                                                            {machine.labels.map(
                                                                (label) => (
                                                                    <span
                                                                        key={
                                                                            label
                                                                        }
                                                                        className="inline-flex items-center rounded bg-white/5 px-2 py-0.5 font-mono text-xs text-white/50"
                                                                    >
                                                                        {label}
                                                                    </span>
                                                                ),
                                                            )}
                                                        </>
                                                    )}
                                            </div>
                                        </div>
                                    </div>

                                    {/* Right side: Resources (smaller) */}
                                    <div className="flex flex-shrink-0 items-center gap-4">
                                        {/* CPU Usage */}
                                        <div className="min-w-[95px]">
                                            <div className="mb-0.5 flex items-center justify-between">
                                                <div className="flex items-center gap-0.5">
                                                    <RiCpuLine
                                                        className="size-2.5 text-purple-400"
                                                        aria-hidden="true"
                                                    />
                                                    <p className="font-mono text-[10px] text-white/40">
                                                        CPU
                                                    </p>
                                                </div>
                                                <p className="font-mono text-xs text-white">
                                                    {cpuPercent}%
                                                </p>
                                            </div>
                                            <div className="h-1 overflow-hidden rounded-full bg-white/10">
                                                <div
                                                    className="h-full bg-purple-500 transition-all"
                                                    style={{
                                                        width: `${cpuPercent}%`,
                                                    }}
                                                />
                                            </div>
                                            <p className="mt-0.5 font-mono text-[10px] text-white/30">
                                                {machine.cpu > 0
                                                    ? `${machine.cpu_allocated}/${cpuLimit}`
                                                    : 'Unknown'}
                                            </p>
                                        </div>

                                        {/* RAM Usage */}
                                        <div className="min-w-[95px]">
                                            <div className="mb-0.5 flex items-center justify-between">
                                                <div className="flex items-center gap-0.5">
                                                    <RiRam2Line
                                                        className="size-2.5 text-orange-400"
                                                        aria-hidden="true"
                                                    />
                                                    <p className="font-mono text-[10px] text-white/40">
                                                        RAM
                                                    </p>
                                                </div>
                                                <p className="font-mono text-xs text-white">
                                                    {ramPercent}%
                                                </p>
                                            </div>
                                            <div className="h-1 overflow-hidden rounded-full bg-white/10">
                                                <div
                                                    className="h-full bg-orange-500 transition-all"
                                                    style={{
                                                        width: `${ramPercent}%`,
                                                    }}
                                                />
                                            </div>
                                            <p className="mt-0.5 font-mono text-[10px] text-white/30">
                                                {machine.ram_available > 0
                                                    ? `${Math.round(machine.ram_allocated / 1024)}GB/${Math.round(ramLimit / 1024)}GB`
                                                    : 'Unknown'}
                                            </p>
                                        </div>

                                        {/* Arrow icon */}
                                        <RiArrowRightSLine
                                            className="size-5 flex-shrink-0 text-white/30 transition-all group-hover:translate-x-0.5 group-hover:text-white/50"
                                            aria-hidden="true"
                                        />
                                    </div>
                                </div>
                            </motion.button>
                        );
                    })}
                </div>
            )}

            <AddMachineModal
                open={isAddMachineModalOpen}
                onOpenChange={setIsAddMachineModalOpen}
            />
        </motion.div>
    );
}
