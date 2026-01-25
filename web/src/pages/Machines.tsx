import { AddMachineModal } from '@/components/AddMachineModal';
import { StatCard } from '@/components/StatCard';
import { useInstallation } from '@/hooks/useInstallation';
import { useMachines } from '@/hooks/useMachines';
import { cx } from '@/lib/utils';
import { MembershipRoleAdmin } from '@/types/installation';
import {
    MachineStatusOffline,
    MachineStatusOnline,
    MachineStatusPaused,
} from '@/types/machine';
import type { Machine } from '@/types/machine';
import {
    RiAddLine,
    RiArrowDownLine,
    RiArrowUpLine,
    RiCheckboxCircleLine,
    RiCloseCircleLine,
    RiCpuLine,
    RiDownloadLine,
    RiPauseLine,
    RiRam2Line,
    RiServerLine,
    RiTimeLine,
} from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useMemo, useState } from 'react';

type StatusFilter = 'all' | 'online' | 'offline' | 'paused';
type SortKey = 'name' | 'status' | 'runners' | 'cpu' | 'ram' | 'lastSeen' | 'arch' | 'labels';
type SortDir = 'asc' | 'desc';

interface Column {
    key: SortKey;
    label: string;
    align?: 'left' | 'right';
    required?: boolean;
}

const ALL_COLUMNS: Column[] = [
    { key: 'name', label: 'Machine', align: 'left', required: true },
    { key: 'status', label: 'Status', align: 'left', required: true },
    { key: 'arch', label: 'Arch', align: 'left' },
    { key: 'runners', label: 'Runners', align: 'right' },
    { key: 'cpu', label: 'CPU', align: 'right' },
    { key: 'ram', label: 'Memory', align: 'right' },
    { key: 'labels', label: 'Labels', align: 'left' },
    { key: 'lastSeen', label: 'Last Seen', align: 'right' },
];

const DEFAULT_VISIBLE_COLUMNS = new Set<SortKey>(['name', 'status', 'runners', 'cpu', 'ram', 'lastSeen']);

export function MachinesPage() {
    const navigate = useNavigate();
    const { selectedInstallation } = useInstallation();
    const { data: machines = [], isLoading, error } = useMachines();
    const [selectedStatus, setSelectedStatus] = useState<StatusFilter>('all');
    const [isAddMachineModalOpen, setIsAddMachineModalOpen] = useState(false);
    const [sortBy, setSortBy] = useState<SortKey>('status');
    const [sortDir, setSortDir] = useState<SortDir>('asc');
    const [visibleColumns, setVisibleColumns] = useState<Set<SortKey>>(DEFAULT_VISIBLE_COLUMNS);

    const isAdmin =
        selectedInstallation?.membership?.role === MembershipRoleAdmin;

    // Calculate statistics
    const stats = useMemo(() => {
        const online = machines.filter((m) => m.status === MachineStatusOnline);
        const offline = machines.filter((m) => m.status === MachineStatusOffline);
        const paused = machines.filter((m) => m.status === MachineStatusPaused);

        const totalCPUAllocated = machines.reduce(
            (sum, m) => sum + m.cpu_allocated,
            0,
        );
        const totalCPULimit = machines.reduce((sum, m) => {
            return sum + (m.cpu_limit > 0 ? m.cpu_limit : m.cpu);
        }, 0);

        const totalRAMAllocated = machines.reduce(
            (sum, m) => sum + m.ram_allocated,
            0,
        );
        const totalRAMLimit = machines.reduce((sum, m) => {
            return sum + (m.ram_limit > 0 ? m.ram_limit : m.ram_total);
        }, 0);

        const totalRunners = machines.reduce(
            (sum, m) => sum + (m.runners?.length || 0),
            0,
        );

        return {
            total: machines.length,
            online: online.length,
            offline: offline.length,
            paused: paused.length,
            cpuAllocated: totalCPUAllocated,
            cpuLimit: totalCPULimit,
            ramAllocated: totalRAMAllocated,
            ramLimit: totalRAMLimit,
            totalRunners,
        };
    }, [machines]);

    // Filter machines
    const filteredMachines = useMemo(() => {
        if (selectedStatus === 'all') return machines;
        return machines.filter((m) => m.status === selectedStatus);
    }, [machines, selectedStatus]);

    // Sort machines
    const sortedMachines = useMemo(() => {
        return [...filteredMachines].sort((a, b) => {
            let comparison = 0;

            switch (sortBy) {
                case 'name':
                    comparison = a.name.localeCompare(b.name);
                    break;
                case 'status': {
                    const statusOrder = { online: 0, paused: 1, offline: 2 };
                    comparison =
                        (statusOrder[a.status as keyof typeof statusOrder] ?? 3) -
                        (statusOrder[b.status as keyof typeof statusOrder] ?? 3);
                    break;
                }
                case 'arch':
                    comparison = a.arch.localeCompare(b.arch);
                    break;
                case 'runners':
                    comparison = (a.runners?.length || 0) - (b.runners?.length || 0);
                    break;
                case 'cpu': {
                    const aCpuPercent = a.cpu_limit > 0 ? a.cpu_allocated / a.cpu_limit : a.cpu > 0 ? a.cpu_allocated / a.cpu : 0;
                    const bCpuPercent = b.cpu_limit > 0 ? b.cpu_allocated / b.cpu_limit : b.cpu > 0 ? b.cpu_allocated / b.cpu : 0;
                    comparison = aCpuPercent - bCpuPercent;
                    break;
                }
                case 'ram': {
                    const aRamPercent = a.ram_limit > 0 ? a.ram_allocated / a.ram_limit : a.ram_total > 0 ? a.ram_allocated / a.ram_total : 0;
                    const bRamPercent = b.ram_limit > 0 ? b.ram_allocated / b.ram_limit : b.ram_total > 0 ? b.ram_allocated / b.ram_total : 0;
                    comparison = aRamPercent - bRamPercent;
                    break;
                }
                case 'labels':
                    comparison = (a.labels?.length || 0) - (b.labels?.length || 0);
                    break;
                case 'lastSeen':
                    comparison = a.last_seen_at - b.last_seen_at;
                    break;
            }

            return sortDir === 'asc' ? comparison : -comparison;
        });
    }, [filteredMachines, sortBy, sortDir]);

    const handleSort = (key: SortKey) => {
        if (sortBy === key) {
            setSortDir(sortDir === 'asc' ? 'desc' : 'asc');
        } else {
            setSortBy(key);
            setSortDir('asc');
        }
    };

    const toggleColumn = (key: SortKey) => {
        const column = ALL_COLUMNS.find(c => c.key === key);
        if (column?.required) return;

        setVisibleColumns(prev => {
            const next = new Set(prev);
            if (next.has(key)) {
                next.delete(key);
            } else {
                next.add(key);
            }
            return next;
        });
    };

    const handleMachineClick = (machineName: string) => {
        navigate({
            to: '/$login/machines/$name',
            params: { login: selectedInstallation!.login, name: machineName },
        });
    };

    const getStatusCount = (status: StatusFilter) => {
        if (status === 'all') return machines.length;
        return machines.filter((m) => m.status === status).length;
    };

    const formatRelativeTime = (timestamp: number) => {
        if (timestamp === 0) return 'Never';
        const now = Date.now() / 1000;
        const diff = now - timestamp;
        if (diff < 60) return 'Just now';
        if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
        if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
        return `${Math.floor(diff / 86400)}d ago`;
    };

    const handleExport = () => {
        const activeColumns = ALL_COLUMNS.filter(col => visibleColumns.has(col.key));
        const headers = activeColumns.map(col => col.label).join(',');
        const rows = sortedMachines.map(machine => {
            const cpuLimit = machine.cpu_limit > 0 ? machine.cpu_limit : machine.cpu;
            const ramLimit = machine.ram_limit > 0 ? machine.ram_limit : machine.ram_total;
            return activeColumns.map(col => {
                switch (col.key) {
                    case 'name': return machine.name;
                    case 'status': return machine.status;
                    case 'arch': return machine.arch;
                    case 'runners': return machine.runners?.length || 0;
                    case 'cpu': return `${machine.cpu_allocated}/${cpuLimit}`;
                    case 'ram': return `${Math.round(machine.ram_allocated / 1024)}/${Math.round(ramLimit / 1024)}`;
                    case 'labels': return (machine.labels || []).join(';');
                    case 'lastSeen': return machine.last_seen_at;
                    default: return '';
                }
            }).join(',');
        }).join('\n');

        const csv = `${headers}\n${rows}`;
        const blob = new Blob([csv], { type: 'text/csv' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `machines-${selectedStatus}-${new Date().toISOString().split('T')[0]}.csv`;
        a.click();
        URL.revokeObjectURL(url);
    };

    if (isLoading) {
        return (
            <div className="space-y-8">
                <div>
                    <h1 className="font-display text-3xl text-white">Machines</h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Manage your self-hosted runners infrastructure
                    </p>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    {[...Array(4)].map((_, i) => (
                        <div
                            key={i}
                            className="h-32 animate-pulse rounded-xl bg-white/[0.02] ring-1 ring-white/5"
                        />
                    ))}
                </div>
                <div className="animate-pulse rounded-xl bg-white/[0.02] ring-1 ring-white/5 h-96" />
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

    const activeColumns = ALL_COLUMNS.filter(col => visibleColumns.has(col.key));

    return (
        <motion.div
            className="space-y-8"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
        >
            {/* Header */}
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="font-display text-3xl text-white">Machines</h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Manage your self-hosted runners infrastructure
                    </p>
                </div>
                {isAdmin && (
                    <button
                        onClick={() => setIsAddMachineModalOpen(true)}
                        className="inline-flex items-center gap-2 rounded-lg bg-white px-4 py-2.5 font-mono text-sm font-medium text-black transition-all hover:bg-white/90"
                    >
                        <RiAddLine className="size-4" />
                        Add Machine
                    </button>
                )}
            </div>

            {/* Stats Cards */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <StatCard
                    label="Total Machines"
                    value={stats.total}
                    subValue={`${stats.online} online, ${stats.offline} offline`}
                    icon={RiServerLine}
                    iconColor="text-blue-400"
                    iconBgColor="bg-blue-500/20"
                    delay={0}
                />
                <StatCard
                    label="Active Runners"
                    value={stats.totalRunners}
                    subValue="Running on your machines"
                    icon={RiCheckboxCircleLine}
                    iconColor="text-emerald-400"
                    iconBgColor="bg-emerald-500/20"
                    delay={0.1}
                />
                <StatCard
                    label="CPU Usage"
                    value={`${stats.cpuLimit > 0 ? Math.round((stats.cpuAllocated / stats.cpuLimit) * 100) : 0}%`}
                    subValue={`${stats.cpuAllocated} / ${stats.cpuLimit} vCPU`}
                    icon={RiCpuLine}
                    iconColor="text-purple-400"
                    iconBgColor="bg-purple-500/20"
                    delay={0.2}
                />
                <StatCard
                    label="Memory Usage"
                    value={`${stats.ramLimit > 0 ? Math.round((stats.ramAllocated / stats.ramLimit) * 100) : 0}%`}
                    subValue={`${Math.round(stats.ramAllocated / 1024)} / ${Math.round(stats.ramLimit / 1024)} GB`}
                    icon={RiRam2Line}
                    iconColor="text-orange-400"
                    iconBgColor="bg-orange-500/20"
                    delay={0.3}
                />
            </div>

            {/* Controls Row */}
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                {/* Left: Filter Dropdown */}
                <div className="flex items-center gap-3">
                    <span className="font-mono text-xs text-white/50">Filter:</span>
                    <select
                        value={selectedStatus}
                        onChange={(e) => setSelectedStatus(e.target.value as StatusFilter)}
                        className="cursor-pointer appearance-none rounded border border-white/10 bg-white/5 px-3 py-1.5 pr-8 font-mono text-xs text-white/70 transition-colors hover:bg-white/10 focus:border-amber-500/50 focus:outline-none"
                        style={{
                            backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.4)' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='6 9 12 15 18 9'%3E%3C/polyline%3E%3C/svg%3E")`,
                            backgroundRepeat: 'no-repeat',
                            backgroundPosition: 'right 8px center',
                        }}
                    >
                        <option value="all">All Machines ({machines.length})</option>
                        <option value="online">Online ({getStatusCount('online')})</option>
                        <option value="offline">Offline ({getStatusCount('offline')})</option>
                        <option value="paused">Paused ({getStatusCount('paused')})</option>
                    </select>
                </div>

                {/* Right: Column Toggles & Export */}
                <div className="flex flex-wrap items-center gap-3">
                    {/* Column Toggles */}
                    <div className="hidden items-center gap-1.5 md:flex">
                        <span className="mr-1 font-mono text-xs text-white/50">Columns:</span>
                        {ALL_COLUMNS.map(col => (
                            <button
                                key={col.key}
                                onClick={() => toggleColumn(col.key)}
                                disabled={col.required}
                                className={cx(
                                    'whitespace-nowrap rounded border px-2 py-1 font-mono text-[11px] transition-colors',
                                    visibleColumns.has(col.key)
                                        ? 'border-cyan-500/30 bg-cyan-500/20 text-cyan-400'
                                        : 'border-white/10 bg-white/5 text-white/50 hover:bg-white/10',
                                    col.required && 'cursor-not-allowed opacity-50'
                                )}
                            >
                                {col.label}
                            </button>
                        ))}
                    </div>

                    {/* Export Button */}
                    <button
                        onClick={handleExport}
                        className="inline-flex items-center gap-1.5 rounded border border-white/10 bg-white/5 px-3 py-1.5 font-mono text-xs text-white/70 transition-colors hover:bg-white/10"
                    >
                        <RiDownloadLine className="size-3.5" />
                        Export
                    </button>
                </div>
            </div>

            {/* Machine Table */}
            {sortedMachines.length === 0 ? (
                <EmptyState
                    hasNoMachines={machines.length === 0}
                    selectedStatus={selectedStatus}
                    isAdmin={isAdmin}
                    onAddMachine={() => setIsAddMachineModalOpen(true)}
                />
            ) : (
                <motion.div
                    className="overflow-hidden rounded-xl border border-white/10 bg-white/[0.02]"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.2 }}
                >
                    <div className="overflow-x-auto">
                        <table className="w-full min-w-[600px]">
                            <thead>
                                <tr className="border-b border-white/10 bg-white/[0.02]">
                                    {activeColumns.map((col) => (
                                        <th
                                            key={col.key}
                                            onClick={() => handleSort(col.key)}
                                            className={cx(
                                                'px-4 py-3 font-mono text-[11px] uppercase tracking-wider text-white/60 transition-colors cursor-pointer hover:text-white',
                                                col.align === 'right' ? 'text-right' : 'text-left'
                                            )}
                                        >
                                            <div className={cx('flex items-center gap-1', col.align === 'right' && 'justify-end')}>
                                                {col.label}
                                                {sortBy === col.key && (
                                                    <span className="text-amber-400">
                                                        {sortDir === 'asc' ? (
                                                            <RiArrowUpLine className="size-3" />
                                                        ) : (
                                                            <RiArrowDownLine className="size-3" />
                                                        )}
                                                    </span>
                                                )}
                                            </div>
                                        </th>
                                    ))}
                                </tr>
                            </thead>
                            <tbody>
                                {sortedMachines.map((machine) => (
                                    <MachineRow
                                        key={machine.name}
                                        machine={machine}
                                        visibleColumns={visibleColumns}
                                        onClick={() => handleMachineClick(machine.name)}
                                        formatRelativeTime={formatRelativeTime}
                                    />
                                ))}
                            </tbody>
                            {sortedMachines.length > 0 && (
                                <tfoot>
                                    <tr className="border-t border-white/10 bg-white/[0.03]">
                                        {activeColumns.map((col, idx) => (
                                            <td
                                                key={col.key}
                                                className={cx(
                                                    'px-4 py-3 font-mono text-xs',
                                                    col.align === 'right' ? 'text-right' : 'text-left'
                                                )}
                                            >
                                                {idx === 0 ? (
                                                    <span className="text-white/60">Total ({sortedMachines.length})</span>
                                                ) : col.key === 'runners' ? (
                                                    <span className="text-white">{stats.totalRunners}</span>
                                                ) : col.key === 'cpu' ? (
                                                    <span className="text-white">{stats.cpuAllocated} / {stats.cpuLimit}</span>
                                                ) : col.key === 'ram' ? (
                                                    <span className="text-white">{Math.round(stats.ramAllocated / 1024)} / {Math.round(stats.ramLimit / 1024)} GB</span>
                                                ) : (
                                                    <span className="text-white/40">-</span>
                                                )}
                                            </td>
                                        ))}
                                    </tr>
                                </tfoot>
                            )}
                        </table>
                    </div>
                </motion.div>
            )}

            <AddMachineModal
                open={isAddMachineModalOpen}
                onOpenChange={setIsAddMachineModalOpen}
            />
        </motion.div>
    );
}

function EmptyState({
    hasNoMachines,
    selectedStatus,
    isAdmin,
    onAddMachine,
}: {
    hasNoMachines: boolean;
    selectedStatus: StatusFilter;
    isAdmin: boolean;
    onAddMachine: () => void;
}) {
    return (
        <motion.div
            className="rounded-xl border border-white/10 bg-white/[0.02] p-12 text-center"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3 }}
        >
            <div className="mx-auto mb-4 flex size-16 items-center justify-center rounded-full bg-white/5">
                <RiServerLine className="size-8 text-white/30" aria-hidden="true" />
            </div>
            <h3 className="font-display text-lg text-white">
                {hasNoMachines ? 'No machines yet' : `No ${selectedStatus} machines`}
            </h3>
            <p className="mx-auto mt-2 max-w-md font-mono text-sm text-white/50">
                {hasNoMachines
                    ? 'Get started by registering a machine to run your CI/CD workflows.'
                    : `No machines are currently ${selectedStatus}.`}
            </p>
            {isAdmin && hasNoMachines && (
                <button
                    onClick={onAddMachine}
                    className="mt-6 inline-flex items-center gap-2 rounded-lg bg-white px-4 py-2.5 font-mono text-sm font-medium text-black transition-all hover:bg-white/90"
                >
                    <RiAddLine className="size-4" />
                    Add Your First Machine
                </button>
            )}
        </motion.div>
    );
}

function MachineRow({
    machine,
    visibleColumns,
    onClick,
    formatRelativeTime,
}: {
    machine: Machine;
    visibleColumns: Set<SortKey>;
    onClick: () => void;
    formatRelativeTime: (timestamp: number) => string;
}) {
    const runnerCount = machine.runners?.length || 0;
    const cpuLimit = machine.cpu_limit > 0 ? machine.cpu_limit : machine.cpu;
    const ramLimit = machine.ram_limit > 0 ? machine.ram_limit : machine.ram_total;
    const cpuPercent = cpuLimit > 0 ? Math.round((machine.cpu_allocated / cpuLimit) * 100) : 0;
    const ramPercent = ramLimit > 0 ? Math.round((machine.ram_allocated / ramLimit) * 100) : 0;

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

    const statusConfig = getStatusConfig(machine.status);

    const getUsageColor = (percent: number) => {
        if (percent >= 90) return 'text-red-400';
        if (percent >= 70) return 'text-amber-400';
        return 'text-white/70';
    };

    return (
        <tr
            onClick={onClick}
            className="border-b border-white/5 transition-colors hover:bg-white/[0.02] cursor-pointer"
        >
            {/* Machine Name */}
            {visibleColumns.has('name') && (
                <td className="px-4 py-3">
                    <div className="flex items-center gap-3">
                        <div
                            className={cx(
                                'size-2 rounded-full flex-shrink-0',
                                statusConfig.dot,
                                machine.status === 'online' && 'animate-pulse',
                            )}
                        />
                        <span className="font-mono text-sm text-white truncate">
                            {machine.name}
                        </span>
                    </div>
                </td>
            )}

            {/* Status */}
            {visibleColumns.has('status') && (
                <td className="px-4 py-3">
                    <span
                        className={cx(
                            'inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs border',
                            statusConfig.bg,
                            statusConfig.border,
                        )}
                    >
                        <statusConfig.icon className={cx('size-3', statusConfig.color)} />
                        <span className={statusConfig.color}>{statusConfig.label}</span>
                    </span>
                </td>
            )}

            {/* Arch */}
            {visibleColumns.has('arch') && (
                <td className="px-4 py-3">
                    <span className="rounded bg-white/5 px-1.5 py-0.5 font-mono text-[10px] text-white/50">
                        {machine.arch}
                    </span>
                </td>
            )}

            {/* Runners */}
            {visibleColumns.has('runners') && (
                <td className="px-4 py-3 text-right">
                    <span className={cx('font-mono text-xs', runnerCount > 0 ? 'text-white' : 'text-white/40')}>
                        {runnerCount}
                    </span>
                </td>
            )}

            {/* CPU */}
            {visibleColumns.has('cpu') && (
                <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-2">
                        <div className="w-16 h-1.5 rounded-full bg-white/10 overflow-hidden">
                            <div
                                className={cx(
                                    'h-full rounded-full transition-all',
                                    cpuPercent >= 90 ? 'bg-red-500' : cpuPercent >= 70 ? 'bg-amber-500' : 'bg-purple-500'
                                )}
                                style={{ width: `${Math.min(cpuPercent, 100)}%` }}
                            />
                        </div>
                        <span className={cx('font-mono text-xs w-16 text-right', getUsageColor(cpuPercent))}>
                            {machine.cpu_allocated}/{cpuLimit}
                        </span>
                    </div>
                </td>
            )}

            {/* Memory */}
            {visibleColumns.has('ram') && (
                <td className="px-4 py-3 text-right">
                    <div className="flex items-center justify-end gap-2">
                        <div className="w-16 h-1.5 rounded-full bg-white/10 overflow-hidden">
                            <div
                                className={cx(
                                    'h-full rounded-full transition-all',
                                    ramPercent >= 90 ? 'bg-red-500' : ramPercent >= 70 ? 'bg-amber-500' : 'bg-orange-500'
                                )}
                                style={{ width: `${Math.min(ramPercent, 100)}%` }}
                            />
                        </div>
                        <span className={cx('font-mono text-xs w-20 text-right', getUsageColor(ramPercent))}>
                            {Math.round(machine.ram_allocated / 1024)}/{Math.round(ramLimit / 1024)}GB
                        </span>
                    </div>
                </td>
            )}

            {/* Labels */}
            {visibleColumns.has('labels') && (
                <td className="px-4 py-3">
                    {machine.labels && machine.labels.length > 0 ? (
                        <div className="flex flex-wrap gap-1">
                            {machine.labels.slice(0, 2).map((label) => (
                                <span
                                    key={label}
                                    className="rounded bg-blue-500/10 px-1.5 py-0.5 font-mono text-[10px] text-blue-400"
                                >
                                    {label}
                                </span>
                            ))}
                            {machine.labels.length > 2 && (
                                <span className="font-mono text-[10px] text-white/30">
                                    +{machine.labels.length - 2}
                                </span>
                            )}
                        </div>
                    ) : (
                        <span className="font-mono text-xs text-white/30">-</span>
                    )}
                </td>
            )}

            {/* Last Seen */}
            {visibleColumns.has('lastSeen') && (
                <td className="px-4 py-3 text-right">
                    <span className="flex items-center justify-end gap-1 font-mono text-xs text-white/50">
                        <RiTimeLine className="size-3" />
                        {formatRelativeTime(machine.last_seen_at)}
                    </span>
                </td>
            )}
        </tr>
    );
}
