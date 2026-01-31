import { Suspense, useState, useMemo, useCallback, useEffect } from 'react';
import { useParams, useNavigate, useSearch } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import * as Dialog from '@radix-ui/react-dialog';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
    RiServerLine,
    RiCpuLine,
    RiDatabase2Line,
    RiAddLine,
    RiAlertLine,
    RiCloseLine,
    RiArrowUpSLine,
    RiArrowDownSLine,
    RiDownloadLine,
    RiMore2Line,
} from '@remixicon/react';
import { PageContainer } from '@/components/PageContainer';
import { Card } from '@/components/Card';
import { Button } from '@/components/Button';
import { EmptyState, LoadingState } from '@/components/PageState';
import { MachineStatusBadge } from '@/components/MachineStatusBadge';
import { ProgressBar } from '@/components/ProgressBar';
import { Badge } from '@/components/Badge';
import { formatBytes, formatTimestamp } from '@/lib/utils';
import type { APIResponse, Machine, Runner } from '@/types/api';
import { CreateMachineDialog } from '@/components/CreateMachineDialog';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/Table';
import { RunnerStatusBadge } from '@/components/RunnerStatusBadge';
import { AppHeader } from '@/components/AppHeader';

const MIB_TO_BYTES = 1024 * 1024;
const formatMib = (mib: number) => formatBytes(mib * MIB_TO_BYTES);
const formatMibToGB = (mib: number) => Math.round((mib / 1024) * 100) / 100;
const toCsvValue = (value: string | number) => {
    const stringValue = String(value ?? '');
    if (/["\n,]/.test(stringValue)) {
        return `"${stringValue.replace(/"/g, '""')}"`;
    }
    return stringValue;
};

interface ViewRunnersDrawerProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    owner: string;
    machineName: string;
}

function ViewRunnersDrawer({ open, onOpenChange, owner, machineName }: ViewRunnersDrawerProps) {
    const { data: response, isLoading } = useQuery<APIResponse<Runner[]>>({
        queryKey: ['machine-runners', owner, machineName],
        queryFn: async () => {
            const res = await fetch(`/api/installations/${owner}/machines/${machineName}/runners`);
            if (!res.ok) throw new Error('Failed to fetch runners');
            return res.json();
        },
        enabled: !!owner && !!machineName && open,
    });

    const runners = response?.data || [];

    return (
        <Dialog.Root open={open} onOpenChange={onOpenChange}>
            <Dialog.Portal>
                <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm animate-fade-in" />
                <Dialog.Content className="fixed right-0 top-0 z-50 h-full w-full max-w-2xl overflow-y-auto border-l border-white/10 bg-[#050507] p-6 shadow-2xl animate-slide-in-right">
                    <Dialog.Title className="mb-2 font-display text-xl text-white">
                        Runners on {machineName}
                    </Dialog.Title>
                    <Dialog.Description className="mb-6 font-mono text-sm text-muted">
                        View all runners currently on this machine.
                    </Dialog.Description>

                    {isLoading ? (
                        <LoadingState />
                    ) : runners.length === 0 ? (
                        <EmptyState
                            icon={RiServerLine}
                            title="No runners"
                            description="This machine has no runners yet."
                        />
                    ) : (
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHead>Status</TableHead>
                                    <TableHead>Name</TableHead>
                                    <TableHead>CPU</TableHead>
                                    <TableHead>RAM</TableHead>
                                    <TableHead>Labels</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {runners.map((runner) => (
                                    <TableRow key={runner.name}>
                                        <TableCell>
                                            <RunnerStatusBadge
                                                status={runner.status}
                                                cancelled={runner.cancelled > 0}
                                            />
                                        </TableCell>
                                        <TableCell>{runner.name}</TableCell>
                                        <TableCell>{runner.cpu}</TableCell>
                                        <TableCell>{formatMib(runner.ram)}</TableCell>
                                        <TableCell>
                                            <div className="flex flex-wrap gap-1">
                                                {runner.labels.slice(0, 2).map((label) => (
                                                    <span
                                                        key={label}
                                                        className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-white/5 text-white/60"
                                                    >
                                                        {label}
                                                    </span>
                                                ))}
                                                {runner.labels.length > 2 && (
                                                    <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-white/5 text-white/60">
                                                        +{runner.labels.length - 2}
                                                    </span>
                                                )}
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    )}

                    <Dialog.Close asChild>
                        <button
                            className="absolute right-4 top-4 rounded-md p-1 text-white/40 hover:bg-white/10 hover:text-white/70"
                            aria-label="Close"
                        >
                            <RiCloseLine className="size-5" />
                        </button>
                    </Dialog.Close>
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}

type ColumnKey = 'name' | 'status' | 'arch' | 'cpu' | 'ram' | 'labels' | 'lastSeen';
type SortKey = 'name' | 'status' | 'cpu' | 'ram' | 'lastSeen';

interface ColumnConfig {
    key: ColumnKey;
    label: string;
    align?: 'left' | 'right';
    sortable?: boolean;
}

const columns: ColumnConfig[] = [
    { key: 'name', label: 'Machine', align: 'left', sortable: true },
    { key: 'status', label: 'Status', align: 'left', sortable: true },
    { key: 'arch', label: 'Arch', align: 'left', sortable: false },
    { key: 'cpu', label: 'CPU', align: 'right', sortable: true },
    { key: 'ram', label: 'RAM', align: 'right', sortable: true },
    { key: 'labels', label: 'Labels', align: 'left', sortable: false },
    { key: 'lastSeen', label: 'Last Seen', align: 'right', sortable: true },
];

const DEFAULT_COLUMNS: ColumnKey[] = ['name', 'status', 'cpu', 'ram', 'lastSeen'];

interface UpdateMachineDialogProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    owner: string;
    machine: Machine | null;
    onSuccess: () => void;
}

function UpdateMachineDialog({ open, onOpenChange, owner, machine, onSuccess }: UpdateMachineDialogProps) {
    const [status, setStatus] = useState<'online' | 'paused'>('online');
    const [cpuLimit, setCpuLimit] = useState('');
    const [ramLimit, setRamLimit] = useState('');
    const [isSaving, setIsSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (!machine) return;
        setStatus(machine.status === 'paused' ? 'paused' : 'online');
        setCpuLimit(machine.cpu_limit > 0 ? String(machine.cpu_limit) : '');
        setRamLimit(machine.ram_limit > 0 ? String(Math.round((machine.ram_limit / 1024) * 100) / 100) : '');
        setError(null);
    }, [machine]);

    const handleSave = async () => {
        if (!machine) return;
        setIsSaving(true);
        setError(null);

        const cpuValue = cpuLimit.trim() === '' ? 0 : Number(cpuLimit);
        const ramGbValue = ramLimit.trim() === '' ? 0 : Number(ramLimit);

        if (Number.isNaN(cpuValue) || cpuValue < 0) {
            setError('CPU limit must be a valid number.');
            setIsSaving(false);
            return;
        }
        if (Number.isNaN(ramGbValue) || ramGbValue < 0) {
            setError('RAM limit must be a valid number.');
            setIsSaving(false);
            return;
        }

        const payload: Record<string, unknown> = {
            status,
            limit: {
                cpu: Math.round(cpuValue),
                ram: Math.round(ramGbValue * 1024),
            },
        };

        try {
            const res = await fetch(`/api/installations/${owner}/machines/${machine.name}`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
            });
            if (!res.ok) {
                const data = await res.json().catch(() => null);
                throw new Error(data?.reason || 'Failed to update machine');
            }
            onSuccess();
            onOpenChange(false);
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Failed to update machine');
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <Dialog.Root open={open} onOpenChange={onOpenChange}>
            <Dialog.Portal>
                <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm animate-fade-in" />
                <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-white/10 bg-[#050507] p-6 shadow-2xl animate-fade-in">
                    <Dialog.Title className="mb-2 font-display text-xl text-white">
                        Update Machine
                    </Dialog.Title>
                    <Dialog.Description className="mb-6 font-mono text-sm text-muted">
                        Update limits and status for <span className="text-white font-semibold">{machine?.name}</span>.
                    </Dialog.Description>

                    <div className="space-y-4">
                        <div>
                            <label className="mb-2 block font-mono text-[10px] uppercase tracking-wider text-white/50">
                                Status
                            </label>
                            <select
                                value={status}
                                onChange={(e) => setStatus(e.target.value as 'online' | 'paused')}
                                className="w-full rounded-md border border-white/10 bg-[#0a0a0c] px-3 py-2 font-mono text-xs text-white/70 focus:border-white/30 focus:outline-none"
                            >
                                <option value="online" className="bg-[#0a0a0c]">Online</option>
                                <option value="paused" className="bg-[#0a0a0c]">Paused</option>
                            </select>
                        </div>

                        <div>
                            <label className="mb-2 block font-mono text-[10px] uppercase tracking-wider text-white/50">
                                CPU Limit (vCPU)
                            </label>
                            <input
                                type="number"
                                min="0"
                                value={cpuLimit}
                                onChange={(e) => setCpuLimit(e.target.value)}
                                placeholder="0 for unlimited"
                                className="w-full rounded-md border border-white/10 bg-[#0a0a0c] px-3 py-2 font-mono text-xs text-white/70 placeholder:text-white/30 focus:border-white/30 focus:outline-none"
                            />
                        </div>

                        <div>
                            <label className="mb-2 block font-mono text-[10px] uppercase tracking-wider text-white/50">
                                RAM Limit (GB)
                            </label>
                            <input
                                type="number"
                                min="0"
                                step="0.5"
                                value={ramLimit}
                                onChange={(e) => setRamLimit(e.target.value)}
                                placeholder="0 for unlimited"
                                className="w-full rounded-md border border-white/10 bg-[#0a0a0c] px-3 py-2 font-mono text-xs text-white/70 placeholder:text-white/30 focus:border-white/30 focus:outline-none"
                            />
                        </div>
                    </div>

                    {error && (
                        <div className="mt-4 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2">
                            <p className="font-mono text-xs text-red-400">{error}</p>
                        </div>
                    )}

                    <div className="mt-6 flex gap-3">
                        <button
                            onClick={() => onOpenChange(false)}
                            className="flex-1 rounded-lg border border-white/10 bg-white/5 px-4 py-2 font-mono text-xs text-white/70 transition-colors hover:bg-white/10 hover:text-white"
                            disabled={isSaving}
                        >
                            Cancel
                        </button>
                        <button
                            onClick={handleSave}
                            className="flex-1 rounded-lg border border-amber-500/30 bg-amber-500/20 px-4 py-2 font-mono text-xs text-amber-400 transition-colors hover:bg-amber-500/30"
                            disabled={isSaving}
                        >
                            {isSaving ? 'Saving...' : 'Save Changes'}
                        </button>
                    </div>

                    <Dialog.Close asChild>
                        <button
                            className="absolute right-4 top-4 rounded-md p-1 text-white/40 hover:bg-white/10 hover:text-white/70"
                            aria-label="Close"
                        >
                            <RiCloseLine className="size-5" />
                        </button>
                    </Dialog.Close>
                </Dialog.Content>
            </Dialog.Portal>
        </Dialog.Root>
    );
}

function MachinePageContent() {
    const { owner } = useParams({ from: '/installations/$owner/machines' });
    const navigate = useNavigate();
    const search = useSearch({ from: '/installations/$owner/machines' });
    const searchParams = search as Record<string, string>;

    const [showCreateDialog, setShowCreateDialog] = useState(false);
    const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
    const [updateTarget, setUpdateTarget] = useState<Machine | null>(null);
    const [isDeleting, setIsDeleting] = useState(false);
    const [viewRunnersFor, setViewRunnersFor] = useState<string | null>(null);

    // Parse visible columns from URL
    const visibleColumns = useMemo(() => {
        const colsParam = searchParams.cols;
        if (colsParam) {
            const cols = colsParam.split(',').filter(c =>
                columns.some(col => col.key === c)
            ) as ColumnKey[];
            return new Set(cols.length > 0 ? cols : DEFAULT_COLUMNS);
        }
        return new Set(DEFAULT_COLUMNS);
    }, [searchParams.cols]);

    // Parse sort from URL
    const sortBy = (searchParams.sort as SortKey) || 'name';
    const sortDir = (searchParams.dir as 'asc' | 'desc') || 'asc';

    const {
        data: response,
        isLoading,
        error,
        refetch,
    } = useQuery<APIResponse<Machine[]>>({
        queryKey: ['machines', owner],
        queryFn: async () => {
            const res = await fetch(`/api/installations/${owner}/machines`);
            if (!res.ok) throw new Error('Failed to fetch machines');
            return res.json();
        },
        enabled: !!owner,
        refetchInterval: 5000,
    });

    const machines = response?.data || [];

    const stats = {
        total: machines.length,
        online: machines.filter((m) => m.status === 'online').length,
        totalCPU: machines.reduce((sum, m) => sum + m.cpu, 0),
        allocatedCPU: machines.reduce((sum, m) => sum + m.cpu_allocated, 0),
        totalRAM: machines.reduce((sum, m) => sum + m.ram_total, 0),
        allocatedRAM: machines.reduce((sum, m) => sum + m.ram_allocated, 0),
    };

    // Sort machines
    const sortedMachines = useMemo(() => {
        return [...machines].sort((a, b) => {
            let aVal: any;
            let bVal: any;

            switch (sortBy) {
                case 'name':
                    aVal = a.name.toLowerCase();
                    bVal = b.name.toLowerCase();
                    break;
                case 'status':
                    aVal = a.status;
                    bVal = b.status;
                    break;
                case 'cpu':
                    aVal = a.cpu_allocated / a.cpu;
                    bVal = b.cpu_allocated / b.cpu;
                    break;
                case 'ram':
                    aVal = a.ram_allocated / a.ram_total;
                    bVal = b.ram_allocated / b.ram_total;
                    break;
                case 'lastSeen':
                    aVal = a.last_seen_at;
                    bVal = b.last_seen_at;
                    break;
                default:
                    return 0;
            }

            const cmp = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
            return sortDir === 'asc' ? cmp : -cmp;
        });
    }, [machines, sortBy, sortDir]);

    const handleSort = useCallback((key: SortKey) => {
        const newParams = { ...searchParams };
        if (sortBy === key) {
            newParams.dir = sortDir === 'asc' ? 'desc' : 'asc';
        } else {
            newParams.sort = key;
            newParams.dir = 'asc';
        }

        navigate({
            to: '/installations/$owner/machines',
            params: { owner },
            search: newParams,
        });
    }, [sortBy, sortDir, searchParams, navigate, owner]);

    const toggleColumn = useCallback((key: ColumnKey) => {
        const newVisible = new Set(visibleColumns);
        if (newVisible.has(key)) {
            if (key !== 'name') newVisible.delete(key);
        } else {
            newVisible.add(key);
        }

        const newParams = { ...searchParams };
        const colsArray = Array.from(newVisible);
        const isDefault = colsArray.length === DEFAULT_COLUMNS.length &&
            colsArray.every(c => DEFAULT_COLUMNS.includes(c));

        if (isDefault) {
            delete newParams.cols;
        } else {
            newParams.cols = colsArray.join(',');
        }

        navigate({
            to: '/installations/$owner/machines',
            params: { owner },
            search: newParams,
        });
    }, [visibleColumns, searchParams, navigate, owner]);

    const handleDeleteMachine = async (machineName: string) => {
        setIsDeleting(true);
        try {
            const res = await fetch(`/api/installations/${owner}/machines/${machineName}`, {
                method: 'DELETE',
            });
            if (!res.ok) throw new Error('Failed to delete machine');
            setDeleteTarget(null);
            refetch();
        } catch (err) {
            console.error('Failed to delete machine:', err);
        } finally {
            setIsDeleting(false);
        }
    };

    const activeColumns = columns.filter(c => visibleColumns.has(c.key));
    const handleExportCsv = useCallback(() => {
        if (machines.length === 0) return;

        const headers = [
            'name',
            'status',
            'arch',
            'cpu',
            'cpu_allocated',
            'cpu_limit',
            'ram_total_gb',
            'ram_allocated_gb',
            'ram_limit_gb',
            'labels',
            'last_seen_at',
        ];
        const rows = machines.map((machine) => {
            const lastSeen = machine.last_seen_at
                ? new Date(machine.last_seen_at * 1000).toISOString()
                : '';
            return [
                machine.name,
                machine.status,
                machine.arch,
                machine.cpu,
                machine.cpu_allocated,
                machine.cpu_limit,
                formatMibToGB(machine.ram_total),
                formatMibToGB(machine.ram_allocated),
                formatMibToGB(machine.ram_limit),
                machine.labels.join(';'),
                lastSeen,
            ].map(toCsvValue).join(',');
        });

        const csv = [headers.join(','), ...rows].join('\n');
        const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
        const url = URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;
        link.download = `machines-${owner || 'export'}.csv`;
        link.click();
        URL.revokeObjectURL(url);
    }, [machines, owner]);

    if (!owner) {
        return (
            <div className="min-h-screen bg-[#050507] text-white grid-bg">
                <PageContainer className="py-16">
                    <EmptyState
                        icon={RiServerLine}
                        title="No installation selected"
                        description="Please select an installation to view machines."
                        action={
                            <Button onClick={() => window.location.href = '/'}>
                                Select Installation
                            </Button>
                        }
                    />
                </PageContainer>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-[#050507] text-white grid-bg">
            <AppHeader />

            <PageContainer className="py-8">
                {/* Page Actions */}
                <div className="mb-8 flex flex-col gap-4">
                    <div className="flex flex-wrap items-center justify-between gap-4">
                        <div>
                            <p className="font-mono text-[10px] uppercase tracking-[0.2em] text-white/40">
                                Infrastructure
                            </p>
                            <h1 className="mt-2 font-display text-3xl text-white">Machines</h1>
                            <p className="mt-2 font-mono text-xs text-muted">
                                Manage self-hosted runners for {owner}
                            </p>
                        </div>
                        <Button variant="primary" onClick={() => setShowCreateDialog(true)} className="h-9">
                            <RiAddLine className="size-4" />
                            New Machine
                        </Button>
                    </div>
                </div>

                {isLoading ? (
                    <LoadingState />
                ) : error ? (
                    <EmptyState
                        icon={RiServerLine}
                        title="Failed to load machines"
                        description="There was an error loading the machines list. Please try again."
                        action={
                            <Button variant="secondary" onClick={() => refetch()}>
                                Retry
                            </Button>
                        }
                    />
                ) : machines.length === 0 ? (
                    <EmptyState
                        icon={RiServerLine}
                        title="No machines yet"
                        description="Get started by creating your first machine. Machines host self-hosted GitHub Actions runners."
                        action={
                            <Button variant="primary" onClick={() => setShowCreateDialog(true)}>
                                <RiAddLine className="size-4" />
                                Create Machine
                            </Button>
                        }
                    />
                ) : (
                    <div className="space-y-6">
                        {/* Stats Grid */}
                        <motion.div
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.5 }}
                            className="grid grid-cols-1 lg:grid-cols-3 gap-4"
                        >
                            <Card padding="md" animate delay={0} className="relative overflow-hidden">
                                <div className="absolute left-0 top-0 h-full w-1 bg-amber-500/70" />
                                <div className="flex items-start justify-between">
                                    <div>
                                        <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                                            Total machines
                                        </div>
                                        <div className="text-3xl font-display font-light tracking-tight text-white">
                                            {stats.total}
                                        </div>
                                        <div className="text-xs font-mono text-muted mt-2">
                                            {stats.online} online
                                        </div>
                                    </div>
                                    <div className="rounded-lg bg-amber-500/10 p-2">
                                        <RiServerLine className="size-5 text-amber-400" />
                                    </div>
                                </div>
                            </Card>

                            <Card padding="md" animate delay={0.2} className="relative overflow-hidden">
                                <div className="absolute left-0 top-0 h-full w-1 bg-cyan-500/70" />
                                <div className="flex items-start justify-between">
                                    <div>
                                        <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                                            CPU
                                        </div>
                                        <div className="text-3xl font-display font-light tracking-tight text-white">
                                            {stats.allocatedCPU}/{stats.totalCPU}
                                        </div>
                                        <div className="text-xs font-mono text-muted mt-2">
                                            cores used
                                        </div>
                                    </div>
                                    <div className="rounded-lg bg-cyan-500/10 p-2">
                                        <RiCpuLine className="size-5 text-cyan-400" />
                                    </div>
                                </div>
                                <div className="mt-3">
                                    <ProgressBar
                                        value={stats.allocatedCPU}
                                        max={stats.totalCPU}
                                        color="#06b6d4"
                                    />
                                </div>
                            </Card>

                            <Card padding="md" animate delay={0.3} className="relative overflow-hidden">
                                <div className="absolute left-0 top-0 h-full w-1 bg-purple-500/70" />
                                <div className="flex items-start justify-between">
                                    <div>
                                        <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                                            RAM
                                        </div>
                                        <div className="text-3xl font-display font-light tracking-tight text-white">
                                            {formatMib(stats.allocatedRAM)}
                                        </div>
                                        <div className="text-xs font-mono text-muted mt-2">
                                            of {formatMib(stats.totalRAM)}
                                        </div>
                                    </div>
                                    <div className="rounded-lg bg-purple-500/10 p-2">
                                        <RiDatabase2Line className="size-5 text-purple-400" />
                                    </div>
                                </div>
                                <div className="mt-3">
                                    <ProgressBar
                                        value={stats.allocatedRAM}
                                        max={stats.totalRAM}
                                        color="#a855f7"
                                    />
                                </div>
                            </Card>
                        </motion.div>

                    {/* Column Selector and Export */}
                    <div className="flex items-center justify-between gap-4">
                        <div className="hidden md:flex items-center gap-2 flex-wrap">
                            <span className="font-mono text-[11px] uppercase tracking-wider text-white/40 mr-2">
                                Columns:
                            </span>
                            {columns.map(col => {
                                const isDisabled = col.key === 'name';
                                return (
                                    <button
                                        key={col.key}
                                        onClick={() => !isDisabled && toggleColumn(col.key)}
                                        disabled={isDisabled}
                                        className={`px-2 py-1 rounded font-mono text-[11px] transition-colors whitespace-nowrap ${visibleColumns.has(col.key)
                                                ? 'bg-cyan-500/20 text-cyan-400 border border-cyan-500/30'
                                                : 'bg-white/5 text-muted border border-white/10 hover:bg-white/10'
                                            } ${isDisabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}`}
                                    >
                                        {col.label}
                                    </button>
                                );
                            })}
                        </div>
                        <Button
                            variant="secondary"
                            onClick={handleExportCsv}
                            disabled={machines.length === 0}
                            className="shrink-0 h-8 text-xs"
                        >
                            <RiDownloadLine className="size-4" />
                            Export CSV
                        </Button>
                    </div>

                        {/* Machines Table */}
                        <motion.div
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.5, delay: 0.4 }}
                        >
                            <Card padding="none" className="mt-4 overflow-hidden">
                                <div className="overflow-x-auto">
                                    <table className="w-full min-w-[800px]">
                                        <thead>
                                            <tr className="border-b border-white/10 bg-white/[0.02]">
                                                {activeColumns.map(col => (
                                                    <th
                                                        key={col.key}
                                                        onClick={() => col.sortable && handleSort(col.key as SortKey)}
                                                        className={`px-4 py-3 font-mono text-[10px] uppercase tracking-[0.2em] text-white/50 ${col.align === 'right' ? 'text-right' : 'text-left'
                                                            } ${col.sortable ? 'cursor-pointer hover:text-white transition-colors' : ''
                                                            }`}
                                                    >
                                                        <div className={`flex items-center gap-1 ${col.align === 'right' ? 'justify-end' : ''}`}>
                                                            {col.label}
                                                            {col.sortable && sortBy === col.key && (
                                                                <span className="text-amber-400">
                                                                    {sortDir === 'asc' ? (
                                                                        <RiArrowUpSLine className="size-4" />
                                                                    ) : (
                                                                        <RiArrowDownSLine className="size-4" />
                                                                    )}
                                                                </span>
                                                            )}
                                                        </div>
                                                    </th>
                                                ))}
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {sortedMachines.map((machine, idx) => (
                                                <motion.tr
                                                    key={machine.name}
                                                    initial={{ opacity: 0 }}
                                                    animate={{ opacity: 1 }}
                                                    transition={{ delay: 0.1 + idx * 0.03 }}
                                                    onClick={() => setViewRunnersFor(machine.name)}
                                                    className="group border-b border-white/5 hover:bg-white/[0.02] transition-colors cursor-pointer"
                                                >
                                                    {activeColumns.map(col => {
                                                        if (col.key === 'name') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3 font-mono text-sm text-white">
                                                                    {machine.name}
                                                                </td>
                                                            );
                                                        }
                                                        if (col.key === 'status') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3">
                                                                    <MachineStatusBadge status={machine.status} />
                                                                </td>
                                                            );
                                                        }
                                                        if (col.key === 'arch') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3">
                                                                    <Badge variant="default">{machine.arch}</Badge>
                                                                </td>
                                                            );
                                                        }
                                                        if (col.key === 'cpu') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3 text-right">
                                                                    <div className="flex flex-col items-end gap-1">
                                                                        <span className="font-mono text-xs text-white/70">
                                                                            {machine.cpu_allocated}/{machine.cpu}
                                                                        </span>
                                                                        <div className="w-20">
                                                                            <ProgressBar
                                                                                value={machine.cpu_allocated}
                                                                                max={machine.cpu}
                                                                                color="#06b6d4"
                                                                            />
                                                                        </div>
                                                                    </div>
                                                                </td>
                                                            );
                                                        }
                                                        if (col.key === 'ram') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3 text-right">
                                                                    <div className="flex flex-col items-end gap-1">
                                                                        <span className="font-mono text-xs text-white/70">
                                                                            {formatMib(machine.ram_allocated)} / {formatMib(machine.ram_total)}
                                                                        </span>
                                                                        <div className="w-20">
                                                                            <ProgressBar
                                                                                value={machine.ram_allocated}
                                                                                max={machine.ram_total}
                                                                                color="#a855f7"
                                                                            />
                                                                        </div>
                                                                    </div>
                                                                </td>
                                                            );
                                                        }
                                                        if (col.key === 'labels') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3">
                                                                    <div className="flex flex-wrap gap-1">
                                                                        {machine.labels.slice(0, 2).map(label => (
                                                                            <span
                                                                                key={label}
                                                                                className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-white/5 text-white/60"
                                                                            >
                                                                                {label}
                                                                            </span>
                                                                        ))}
                                                                        {machine.labels.length > 2 && (
                                                                            <span className="px-1.5 py-0.5 rounded text-[10px] font-mono bg-white/5 text-white/60">
                                                                                +{machine.labels.length - 2}
                                                                            </span>
                                                                        )}
                                                                    </div>
                                                                </td>
                                                            );
                                                        }
                                                        if (col.key === 'lastSeen') {
                                                            return (
                                                                <td key={col.key} className="px-4 py-3 text-right font-mono text-xs text-white/50">
                                                                    <div className="flex items-center justify-end gap-2">
                                                                        <span>{formatTimestamp(machine.last_seen_at)}</span>
                                                                        <DropdownMenu.Root>
                                                                            <DropdownMenu.Trigger asChild>
                                                                                <button
                                                                                    onClick={(event) => event.stopPropagation()}
                                                                                    className="rounded-md p-1 text-white/40 hover:bg-white/10 hover:text-white/70"
                                                                                    aria-label="Open machine menu"
                                                                                >
                                                                                    <RiMore2Line className="size-4" />
                                                                                </button>
                                                                            </DropdownMenu.Trigger>
                                                                            <DropdownMenu.Portal>
                                                                                <DropdownMenu.Content
                                                                                    className="z-50 min-w-[200px] rounded-lg border border-white/10 bg-[#050507] p-2 shadow-2xl animate-fade-in"
                                                                                    sideOffset={6}
                                                                                >
                                                                                    <DropdownMenu.Item
                                                                                        onSelect={() => setUpdateTarget(machine)}
                                                                                        className="flex items-center gap-2 rounded-md px-2 py-2 font-mono text-xs text-white/70 outline-none cursor-pointer hover:bg-white/5 focus:bg-white/5"
                                                                                    >
                                                                                        Update Settings
                                                                                    </DropdownMenu.Item>
                                                                                    <DropdownMenu.Item
                                                                                        onSelect={() => setDeleteTarget(machine.name)}
                                                                                        className="flex items-center gap-2 rounded-md px-2 py-2 font-mono text-xs text-red-400 outline-none cursor-pointer hover:bg-red-500/10 focus:bg-red-500/10"
                                                                                    >
                                                                                        Delete Machine
                                                                                    </DropdownMenu.Item>
                                                                                </DropdownMenu.Content>
                                                                            </DropdownMenu.Portal>
                                                                        </DropdownMenu.Root>
                                                                    </div>
                                                                </td>
                                                            );
                                                        }
                                                        return null;
                                                    })}
                                                </motion.tr>
                                            ))}
                                        </tbody>
                                    </table>
                                </div>
                            </Card>
                        </motion.div>
                    </div>
                )}
            </PageContainer>

            {/* Create Machine Dialog */}
            <CreateMachineDialog
                open={showCreateDialog}
                onOpenChange={setShowCreateDialog}
                owner={owner || ''}
                onSuccess={() => refetch()}
            />

            {/* Delete Confirmation Dialog */}
            <Dialog.Root open={!!deleteTarget} onOpenChange={(open) => !open && setDeleteTarget(null)}>
                <Dialog.Portal>
                    <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm animate-fade-in" />
                    <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-white/10 bg-[#050507] p-6 shadow-2xl animate-fade-in">
                        <Dialog.Title className="mb-4 font-display text-xl text-white">
                            Delete Machine
                        </Dialog.Title>
                        <Dialog.Description className="mb-6 font-mono text-sm text-muted">
                            Are you sure you want to delete <span className="text-white font-semibold">{deleteTarget}</span>?
                            This will cancel all runners on this machine.
                        </Dialog.Description>

                        <div className="mb-6 rounded-lg border border-red-500/20 bg-red-500/5 p-4">
                            <div className="flex items-start gap-3">
                                <RiAlertLine className="mt-0.5 size-5 flex-shrink-0 text-red-400" />
                                <p className="font-mono text-xs text-red-400/80">
                                    This action cannot be undone. All runners on this machine will be cancelled.
                                </p>
                            </div>
                        </div>

                        <div className="flex gap-3">
                            <Button
                                variant="secondary"
                                className="flex-1"
                                onClick={() => setDeleteTarget(null)}
                                disabled={isDeleting}
                            >
                                Cancel
                            </Button>
                            <Button
                                variant="danger"
                                className="flex-1"
                                onClick={() => deleteTarget && handleDeleteMachine(deleteTarget)}
                                disabled={isDeleting}
                            >
                                {isDeleting ? 'Deleting...' : 'Delete Machine'}
                            </Button>
                        </div>

                        <Dialog.Close asChild>
                            <button
                                className="absolute right-4 top-4 rounded-md p-1 text-white/40 hover:bg-white/10 hover:text-white/70"
                                aria-label="Close"
                            >
                                <RiCloseLine className="size-5" />
                            </button>
                        </Dialog.Close>
                    </Dialog.Content>
                </Dialog.Portal>
            </Dialog.Root>

            {/* View Runners Drawer */}
            <ViewRunnersDrawer
                open={!!viewRunnersFor}
                onOpenChange={(open) => !open && setViewRunnersFor(null)}
                owner={owner || ''}
                machineName={viewRunnersFor || ''}
            />

            {/* Update Machine Dialog */}
            <UpdateMachineDialog
                open={!!updateTarget}
                onOpenChange={(open) => !open && setUpdateTarget(null)}
                owner={owner || ''}
                machine={updateTarget}
                onSuccess={() => refetch()}
            />
        </div>
    );
}

export default function MachinePage() {
    return (
        <Suspense
            fallback={
                <div className="min-h-screen bg-[#050507] text-white grid-bg flex items-center justify-center">
                    <div className="font-mono text-sm text-white/40">Loading...</div>
                </div>
            }
        >
            <MachinePageContent />
        </Suspense>
    );
}
