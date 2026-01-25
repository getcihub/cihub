import { StatCard } from '@/components/StatCard';
import { useInstallation } from '@/hooks/useInstallation';
import { useRunners } from '@/hooks/useRunners';
import { cx } from '@/lib/utils';
import type { RunnerStatus, RunnerWithJob } from '@/types/runner';
import {
    RiArrowDownLine,
    RiArrowUpLine,
    RiCheckboxCircleLine,
    RiCpuLine,
    RiDownloadLine,
    RiGitBranchLine,
    RiLoader4Line,
    RiPlayCircleLine,
    RiRam2Line,
    RiServerLine,
    RiStopCircleLine,
    RiTimeLine,
} from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useMemo, useState } from 'react';

type StatusFilter = 'all' | 'busy' | 'idle' | 'pending' | 'completed';
type SortKey = 'name' | 'status' | 'machine' | 'cpu' | 'ram' | 'time' | 'arch' | 'labels';
type SortDir = 'asc' | 'desc';

interface Column {
    key: SortKey;
    label: string;
    align?: 'left' | 'right';
    required?: boolean;
}

const ALL_COLUMNS: Column[] = [
    { key: 'name', label: 'Runner', align: 'left', required: true },
    { key: 'status', label: 'Status', align: 'left', required: true },
    { key: 'machine', label: 'Machine', align: 'left' },
    { key: 'arch', label: 'Arch', align: 'left' },
    { key: 'cpu', label: 'CPU', align: 'right' },
    { key: 'ram', label: 'Memory', align: 'right' },
    { key: 'labels', label: 'Labels', align: 'left' },
    { key: 'time', label: 'Time', align: 'right' },
];

const DEFAULT_VISIBLE_COLUMNS = new Set<SortKey>(['name', 'status', 'machine', 'cpu', 'ram', 'time']);

export function RunnersPage() {
    const navigate = useNavigate();
    const { selectedInstallation } = useInstallation();
    const { data: runners = [], isLoading, error } = useRunners();
    const [selectedStatus, setSelectedStatus] = useState<StatusFilter>('all');
    const [sortBy, setSortBy] = useState<SortKey>('status');
    const [sortDir, setSortDir] = useState<SortDir>('asc');
    const [visibleColumns, setVisibleColumns] = useState<Set<SortKey>>(DEFAULT_VISIBLE_COLUMNS);

    // Calculate statistics
    const stats = useMemo(() => {
        const busy = runners.filter((r) => r.status === 'busy');
        const idle = runners.filter((r) => r.status === 'idle');
        const pending = runners.filter((r) => r.status === 'pending');
        const completed = runners.filter((r) => r.status === 'completed');

        const totalCPU = runners.reduce((sum, r) => sum + r.cpu, 0);
        const busyCPU = busy.reduce((sum, r) => sum + r.cpu, 0);
        const totalRAM = runners.reduce((sum, r) => sum + r.ram, 0);
        const busyRAM = busy.reduce((sum, r) => sum + r.ram, 0);

        return {
            total: runners.length,
            busy: busy.length,
            idle: idle.length,
            pending: pending.length,
            completed: completed.length,
            totalCPU,
            busyCPU,
            totalRAM,
            busyRAM,
        };
    }, [runners]);

    // Filter runners
    const filteredRunners = useMemo(() => {
        if (selectedStatus === 'all') return runners;
        return runners.filter((r) => r.status === selectedStatus);
    }, [runners, selectedStatus]);

    // Sort runners
    const sortedRunners = useMemo(() => {
        return [...filteredRunners].sort((a, b) => {
            let comparison = 0;

            switch (sortBy) {
                case 'name':
                    comparison = a.name.localeCompare(b.name);
                    break;
                case 'status': {
                    const statusOrder = { busy: 0, idle: 1, pending: 2, registered: 3, completed: 4 };
                    comparison =
                        (statusOrder[a.status as keyof typeof statusOrder] ?? 5) -
                        (statusOrder[b.status as keyof typeof statusOrder] ?? 5);
                    break;
                }
                case 'machine':
                    comparison = a.machine.localeCompare(b.machine);
                    break;
                case 'arch':
                    comparison = a.arch.localeCompare(b.arch);
                    break;
                case 'cpu':
                    comparison = a.cpu - b.cpu;
                    break;
                case 'ram':
                    comparison = a.ram - b.ram;
                    break;
                case 'labels':
                    comparison = (a.labels?.length || 0) - (b.labels?.length || 0);
                    break;
                case 'time':
                    comparison = a.updated - b.updated;
                    break;
            }

            return sortDir === 'asc' ? comparison : -comparison;
        });
    }, [filteredRunners, sortBy, sortDir]);

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

    const handleRunnerClick = (runnerName: string) => {
        navigate({
            to: '/$login/runners/$name',
            params: { login: selectedInstallation!.login, name: runnerName },
        });
    };

    const getStatusCount = (status: StatusFilter) => {
        if (status === 'all') return runners.length;
        return runners.filter((r) => r.status === status).length;
    };

    const formatDuration = (startTimestamp: number) => {
        if (startTimestamp === 0) return '-';
        const now = Date.now() / 1000;
        const diff = now - startTimestamp;
        if (diff < 60) return `${Math.floor(diff)}s`;
        if (diff < 3600) return `${Math.floor(diff / 60)}m ${Math.floor(diff % 60)}s`;
        return `${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`;
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
        const rows = sortedRunners.map(runner => {
            return activeColumns.map(col => {
                switch (col.key) {
                    case 'name': return runner.name;
                    case 'status': return runner.status;
                    case 'machine': return runner.machine;
                    case 'arch': return runner.arch;
                    case 'cpu': return runner.cpu;
                    case 'ram': return Math.round(runner.ram / 1024);
                    case 'labels': return (runner.labels || []).join(';');
                    case 'time': return runner.updated;
                    default: return '';
                }
            }).join(',');
        }).join('\n');

        const csv = `${headers}\n${rows}`;
        const blob = new Blob([csv], { type: 'text/csv' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `runners-${selectedStatus}-${new Date().toISOString().split('T')[0]}.csv`;
        a.click();
        URL.revokeObjectURL(url);
    };

    if (isLoading) {
        return (
            <div className="space-y-8">
                <div>
                    <h1 className="font-display text-3xl text-white">Runners</h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Active workflow runners across your machines
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
                <h1 className="font-display text-3xl text-white">Runners</h1>
                <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-6">
                    <p className="font-mono text-sm text-red-400">
                        Failed to load runners. Please try again later.
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
            <div>
                <h1 className="font-display text-3xl text-white">Runners</h1>
                <p className="mt-2 font-mono text-sm text-white/50">
                    Active workflow runners across your machines
                </p>
            </div>

            {/* Stats Cards */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <StatCard
                    label="Total Runners"
                    value={stats.total}
                    subValue={`${stats.busy} busy, ${stats.idle} idle`}
                    icon={RiServerLine}
                    iconColor="text-blue-400"
                    iconBgColor="bg-blue-500/20"
                    delay={0}
                />
                <StatCard
                    label="Running Jobs"
                    value={stats.busy}
                    subValue="Currently executing workflows"
                    icon={RiPlayCircleLine}
                    iconColor="text-emerald-400"
                    iconBgColor="bg-emerald-500/20"
                    delay={0.1}
                />
                <StatCard
                    label="CPU In Use"
                    value={`${stats.totalCPU > 0 ? Math.round((stats.busyCPU / stats.totalCPU) * 100) : 0}%`}
                    subValue={`${stats.busyCPU} / ${stats.totalCPU} vCPU`}
                    icon={RiCpuLine}
                    iconColor="text-purple-400"
                    iconBgColor="bg-purple-500/20"
                    delay={0.2}
                />
                <StatCard
                    label="Memory In Use"
                    value={`${stats.totalRAM > 0 ? Math.round((stats.busyRAM / stats.totalRAM) * 100) : 0}%`}
                    subValue={`${Math.round(stats.busyRAM / 1024)} / ${Math.round(stats.totalRAM / 1024)} GB`}
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
                        <option value="all">All Runners ({runners.length})</option>
                        <option value="busy">Busy ({getStatusCount('busy')})</option>
                        <option value="idle">Idle ({getStatusCount('idle')})</option>
                        <option value="pending">Pending ({getStatusCount('pending')})</option>
                        <option value="completed">Completed ({getStatusCount('completed')})</option>
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

            {/* Runners Table */}
            {sortedRunners.length === 0 ? (
                <EmptyState
                    hasNoRunners={runners.length === 0}
                    selectedStatus={selectedStatus}
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
                                {sortedRunners.map((runner) => (
                                    <RunnerRow
                                        key={runner.id}
                                        runner={runner}
                                        visibleColumns={visibleColumns}
                                        onClick={() => handleRunnerClick(runner.name)}
                                        formatDuration={formatDuration}
                                        formatRelativeTime={formatRelativeTime}
                                    />
                                ))}
                            </tbody>
                            {sortedRunners.length > 0 && (
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
                                                    <span className="text-white/60">Total ({sortedRunners.length})</span>
                                                ) : col.key === 'cpu' ? (
                                                    <span className="text-white">{stats.busyCPU} / {stats.totalCPU}</span>
                                                ) : col.key === 'ram' ? (
                                                    <span className="text-white">{Math.round(stats.busyRAM / 1024)} / {Math.round(stats.totalRAM / 1024)} GB</span>
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
        </motion.div>
    );
}

function EmptyState({
    hasNoRunners,
    selectedStatus,
}: {
    hasNoRunners: boolean;
    selectedStatus: StatusFilter;
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
                {hasNoRunners ? 'No runners yet' : `No ${selectedStatus} runners`}
            </h3>
            <p className="mx-auto mt-2 max-w-md font-mono text-sm text-white/50">
                {hasNoRunners
                    ? 'Runners will appear here when workflows are triggered and assigned to your machines.'
                    : `No runners are currently ${selectedStatus}.`}
            </p>
        </motion.div>
    );
}

function RunnerRow({
    runner,
    visibleColumns,
    onClick,
    formatDuration,
    formatRelativeTime,
}: {
    runner: RunnerWithJob;
    visibleColumns: Set<SortKey>;
    onClick: () => void;
    formatDuration: (timestamp: number) => string;
    formatRelativeTime: (timestamp: number) => string;
}) {
    const getStatusConfig = (status: RunnerStatus) => {
        switch (status) {
            case 'busy':
                return {
                    icon: RiPlayCircleLine,
                    color: 'text-emerald-400',
                    bg: 'bg-emerald-500/10',
                    border: 'border-emerald-500/20',
                    dot: 'bg-emerald-500 shadow-lg shadow-emerald-500/50',
                    label: 'Busy',
                    animate: true,
                };
            case 'idle':
                return {
                    icon: RiCheckboxCircleLine,
                    color: 'text-blue-400',
                    bg: 'bg-blue-500/10',
                    border: 'border-blue-500/20',
                    dot: 'bg-blue-500',
                    label: 'Idle',
                    animate: false,
                };
            case 'pending':
                return {
                    icon: RiLoader4Line,
                    color: 'text-amber-400',
                    bg: 'bg-amber-500/10',
                    border: 'border-amber-500/20',
                    dot: 'bg-amber-500',
                    label: 'Pending',
                    animate: true,
                };
            case 'completed':
                return {
                    icon: RiStopCircleLine,
                    color: 'text-white/40',
                    bg: 'bg-white/5',
                    border: 'border-white/10',
                    dot: 'bg-white/30',
                    label: 'Completed',
                    animate: false,
                };
            default:
                return {
                    icon: RiServerLine,
                    color: 'text-white/40',
                    bg: 'bg-white/5',
                    border: 'border-white/10',
                    dot: 'bg-white/30',
                    label: status,
                    animate: false,
                };
        }
    };

    const statusConfig = getStatusConfig(runner.status);

    return (
        <>
            <tr
                onClick={onClick}
                className="border-b border-white/5 transition-colors hover:bg-white/[0.02] cursor-pointer"
            >
                {/* Runner Name */}
                {visibleColumns.has('name') && (
                    <td className="px-4 py-3">
                        <div className="flex items-center gap-3">
                            <div
                                className={cx(
                                    'size-2 rounded-full flex-shrink-0',
                                    statusConfig.dot,
                                    statusConfig.animate && 'animate-pulse',
                                )}
                            />
                            <span className="font-mono text-sm text-white truncate">
                                {runner.name}
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
                            <statusConfig.icon
                                className={cx(
                                    'size-3',
                                    statusConfig.color,
                                    statusConfig.animate && runner.status === 'pending' && 'animate-spin',
                                )}
                            />
                            <span className={statusConfig.color}>{statusConfig.label}</span>
                        </span>
                    </td>
                )}

                {/* Machine */}
                {visibleColumns.has('machine') && (
                    <td className="px-4 py-3">
                        <span className="font-mono text-xs text-white/70">
                            {runner.machine}
                        </span>
                    </td>
                )}

                {/* Arch */}
                {visibleColumns.has('arch') && (
                    <td className="px-4 py-3">
                        <span className="rounded bg-white/5 px-1.5 py-0.5 font-mono text-[10px] text-white/50">
                            {runner.arch}
                        </span>
                    </td>
                )}

                {/* CPU */}
                {visibleColumns.has('cpu') && (
                    <td className="px-4 py-3 text-right">
                        <span className="font-mono text-xs text-white/70">
                            {runner.cpu} vCPU
                        </span>
                    </td>
                )}

                {/* Memory */}
                {visibleColumns.has('ram') && (
                    <td className="px-4 py-3 text-right">
                        <span className="font-mono text-xs text-white/70">
                            {Math.round(runner.ram / 1024)} GB
                        </span>
                    </td>
                )}

                {/* Labels */}
                {visibleColumns.has('labels') && (
                    <td className="px-4 py-3">
                        {runner.labels && runner.labels.length > 0 ? (
                            <div className="flex flex-wrap gap-1">
                                {runner.labels.slice(0, 2).map((label) => (
                                    <span
                                        key={label}
                                        className="rounded bg-blue-500/10 px-1.5 py-0.5 font-mono text-[10px] text-blue-400"
                                    >
                                        {label}
                                    </span>
                                ))}
                                {runner.labels.length > 2 && (
                                    <span className="font-mono text-[10px] text-white/30">
                                        +{runner.labels.length - 2}
                                    </span>
                                )}
                            </div>
                        ) : (
                            <span className="font-mono text-xs text-white/30">-</span>
                        )}
                    </td>
                )}

                {/* Time */}
                {visibleColumns.has('time') && (
                    <td className="px-4 py-3 text-right">
                        <span className="flex items-center justify-end gap-1 font-mono text-xs text-white/50">
                            <RiTimeLine className="size-3" />
                            {runner.status === 'busy'
                                ? formatDuration(runner.started)
                                : formatRelativeTime(runner.updated)}
                        </span>
                    </td>
                )}
            </tr>

            {/* Job Info Row (if busy) */}
            {runner.status === 'busy' && runner.job && (
                <tr
                    onClick={onClick}
                    className="border-b border-white/5 bg-white/[0.01] cursor-pointer hover:bg-white/[0.02]"
                >
                    <td colSpan={Array.from(visibleColumns).length} className="px-4 py-2">
                        <div className="ml-5 flex items-center gap-3 rounded-lg border border-emerald-500/10 bg-emerald-500/5 px-3 py-2">
                            <img
                                src={runner.job.author_avatar}
                                alt={runner.job.author_login}
                                className="size-6 rounded-full border border-white/10"
                            />
                            <div className="min-w-0 flex-1">
                                <p className="truncate font-mono text-xs text-white">
                                    {runner.job.name}
                                </p>
                                <div className="flex items-center gap-2">
                                    <span className="truncate font-mono text-[10px] text-white/50">
                                        {runner.job.repo}
                                    </span>
                                    <span className="flex items-center gap-1 font-mono text-[10px] text-white/40">
                                        <RiGitBranchLine className="size-2.5" />
                                        {runner.job.branch}
                                    </span>
                                </div>
                            </div>
                            <div className="flex-shrink-0 text-right">
                                <span className="font-mono text-xs text-emerald-400">
                                    {formatDuration(runner.job.started_at)}
                                </span>
                            </div>
                        </div>
                    </td>
                </tr>
            )}
        </>
    );
}
