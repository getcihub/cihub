import { useInstallation } from '@/hooks/useInstallation';
import { cx } from '@/lib/utils';
import {
    RiArrowDownSLine,
    RiArrowRightSLine,
    RiArrowUpSLine,
    RiCheckboxCircleLine,
    RiCpuLine,
    RiPlayCircleLine,
    RiRam2Line,
    RiServerLine,
    RiTimeLine,
} from '@remixicon/react';
import { Link } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useMemo } from 'react';
import { useTimeRange } from '@/context/TimeRangeContext';
import { TimeRangeSelector } from '@/components/TimeRangeSelector';

// Mock data toggle
const USE_MOCK_DATA = true;

interface DailyStats {
    date: string;
    runners: number;
    completed: number;
    cancelled: number;
    avgDuration: number;
}

function createMockData(days: number) {
    const now = Date.now() / 1000;
    const oneDay = 86400;

    const dailyStats: DailyStats[] = [];
    for (let i = days - 1; i >= 0; i--) {
        const date = new Date((now - i * oneDay) * 1000);
        const runners = Math.floor(Math.random() * 40) + 15;
        const cancelled = Math.floor(Math.random() * 3);
        dailyStats.push({
            date: date.toLocaleDateString('en-US', { weekday: 'short', month: 'short', day: 'numeric' }),
            runners,
            completed: runners - cancelled,
            cancelled,
            avgDuration: Math.floor(Math.random() * 300) + 60,
        });
    }

    const totalRunners = dailyStats.reduce((sum, d) => sum + d.runners, 0);
    const totalCompleted = dailyStats.reduce((sum, d) => sum + d.completed, 0);
    const totalCancelled = dailyStats.reduce((sum, d) => sum + d.cancelled, 0);
    const avgDuration = Math.round(dailyStats.reduce((sum, d) => sum + d.avgDuration, 0) / dailyStats.length);

    // Previous period for comparison
    const prevTotalRunners = Math.floor(totalRunners * (0.8 + Math.random() * 0.4));
    const prevCompletionRate = Math.floor(90 + Math.random() * 8);
    const prevAvgDuration = Math.floor(avgDuration * (0.9 + Math.random() * 0.2));

    return {
        dailyStats,
        current: {
            totalRunners,
            totalCompleted,
            totalCancelled,
            completionRate: Math.round((totalCompleted / totalRunners) * 100),
            avgDuration,
        },
        previous: {
            totalRunners: prevTotalRunners,
            completionRate: prevCompletionRate,
            avgDuration: prevAvgDuration,
        },
        lifetime: {
            totalRunners: 8472,
            totalMinutes: 38291,
            avgCompletionRate: 96,
        },
        resources: {
            machines: { total: 5, online: 4 },
            runners: { total: 12, busy: 3, idle: 7, pending: 2 },
            cpu: { allocated: 24, limit: 48 },
            ram: { allocated: 64, limit: 128 },
        },
        statusBreakdown: {
            busy: 3,
            idle: 7,
            pending: 2,
            completed: totalCompleted,
        },
    };
}

export function OverviewPage() {
    const { selectedInstallation } = useInstallation();
    const { range, setRange, days, isPending } = useTimeRange();

    const mockData = useMemo(() => {
        if (USE_MOCK_DATA) {
            return createMockData(days);
        }
        return null;
    }, [days]);

    if (!mockData) {
        return (
            <div className="flex items-center justify-center py-20">
                <p className="font-mono text-sm text-white/50">Loading...</p>
            </div>
        );
    }

    const { dailyStats, current, previous, resources, statusBreakdown } = mockData;

    const runnersDiff = current.totalRunners - previous.totalRunners;
    const runnersPercent = previous.totalRunners > 0 ? Math.round((runnersDiff / previous.totalRunners) * 100) : 0;
    const completionDiff = current.completionRate - previous.completionRate;
    const durationDiff = current.avgDuration - previous.avgDuration;

    const formatNumber = (n: number) => n.toLocaleString();
    const formatDuration = (seconds: number) => {
        if (seconds < 60) return `${seconds}s`;
        if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
        return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
    };

    const maxRunners = Math.max(...dailyStats.map(d => d.runners));
    const chartData = dailyStats;

    return (
        <div className="space-y-6">
            {/* Header with Time Range */}
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <h1 className="font-display text-3xl text-white">Overview</h1>
                    <p className="mt-1 font-mono text-sm text-white/50">
                        Real-time snapshot of runner activity and resource utilization.
                    </p>
                </div>
                <TimeRangeSelector value={range} onChange={setRange} isPending={isPending} />
            </div>

            {/* Runner Activity Chart - Full Width */}
            <motion.div
                className="rounded-xl border border-white/10 bg-white/[0.02]"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.1 }}
            >
                <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
                    <div>
                        <h3 className="font-mono text-sm text-white">Runner Activity</h3>
                        <p className="mt-0.5 font-mono text-xs text-white/40">Daily runner executions</p>
                    </div>
                    <div className="flex items-center gap-4">
                        <div className="flex items-center gap-1.5">
                            <div className="size-2.5 rounded-sm bg-emerald-500" />
                            <span className="font-mono text-[10px] text-white/50">Completed</span>
                        </div>
                        <div className="flex items-center gap-1.5">
                            <div className="size-2.5 rounded-sm bg-red-500" />
                            <span className="font-mono text-[10px] text-white/50">Cancelled</span>
                        </div>
                    </div>
                </div>
                <div className="p-5">
                    <div className="flex items-end gap-1" style={{ height: 120 }}>
                        {chartData.map((day, index) => {
                            const height = maxRunners > 0 ? (day.runners / maxRunners) * 100 : 0;
                            const completedRatio = day.runners > 0 ? day.completed / day.runners : 1;
                            return (
                                <div
                                    key={`${day.date}-${index}`}
                                    className="group relative flex flex-1 flex-col items-center"
                                >
                                    <div className="relative w-full" style={{ height: 100 }}>
                                        <motion.div
                                            className="absolute bottom-0 left-1/2 w-full max-w-[24px] -translate-x-1/2 rounded-t-sm bg-red-500/80"
                                            initial={{ height: 0 }}
                                            animate={{ height: `${height}%` }}
                                            transition={{ duration: 0.4, delay: 0.2 + index * 0.015 }}
                                        />
                                        <motion.div
                                            className="absolute bottom-0 left-1/2 w-full max-w-[24px] -translate-x-1/2 rounded-t-sm bg-emerald-500"
                                            initial={{ height: 0 }}
                                            animate={{ height: `${height * completedRatio}%` }}
                                            transition={{ duration: 0.4, delay: 0.25 + index * 0.015 }}
                                        />
                                    </div>
                                    {/* Tooltip */}
                                    <div className="pointer-events-none absolute -top-10 left-1/2 -translate-x-1/2 opacity-0 transition-opacity group-hover:opacity-100">
                                        <div className="whitespace-nowrap rounded bg-white/10 px-2 py-1 font-mono text-[10px] text-white backdrop-blur">
                                            {day.runners} runners ({day.completed} ok)
                                        </div>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                    <div className="mt-2 flex justify-between">
                        <span className="font-mono text-[10px] text-white/30">{chartData[0]?.date}</span>
                        <span className="font-mono text-[10px] text-white/30">{chartData[chartData.length - 1]?.date}</span>
                    </div>
                </div>
            </motion.div>

            {/* Main Stats Grid */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <StatBox
                    icon={RiPlayCircleLine}
                    iconColor="text-cyan-400"
                    label="Total Runners"
                    value={formatNumber(current.totalRunners)}
                    comparison={{ value: runnersPercent, label: 'vs prev period' }}
                    delay={0}
                />
                <StatBox
                    icon={RiCheckboxCircleLine}
                    iconColor="text-emerald-400"
                    label="Completion Rate"
                    value={`${current.completionRate}%`}
                    comparison={{ value: completionDiff, label: 'vs prev period', suffix: 'pts' }}
                    delay={0.05}
                />
                <StatBox
                    icon={RiTimeLine}
                    iconColor="text-purple-400"
                    label="Avg Duration"
                    value={formatDuration(current.avgDuration)}
                    comparison={{ value: -durationDiff, label: 'vs prev period', suffix: 's', invertColors: true }}
                    delay={0.1}
                />
                <StatBox
                    icon={RiServerLine}
                    iconColor="text-orange-400"
                    label="Active Now"
                    value={`${resources.runners.busy + resources.runners.idle}`}
                    subValue={`${resources.runners.busy} busy • ${resources.runners.idle} idle • ${resources.runners.pending} pending`}
                    delay={0.15}
                />
            </div>

            {/* Resource Allocation Strip */}
            <motion.div
                className="rounded-xl border border-white/10 bg-white/[0.02] p-5"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.2 }}
            >
                <div className="mb-4 flex items-center justify-between">
                    <h3 className="font-mono text-sm text-white/70">Resource Allocation</h3>
                    <Link
                        to="/$login/machines"
                        params={{ login: selectedInstallation?.login || '' }}
                        className="flex items-center gap-1 font-mono text-xs text-white/40 transition-colors hover:text-white"
                    >
                        View machines
                        <RiArrowRightSLine className="size-3.5" />
                    </Link>
                </div>
                <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
                    <ResourceBar
                        icon={RiServerLine}
                        iconColor="text-blue-400"
                        barColor="bg-blue-500"
                        label="Machines"
                        current={resources.machines.online}
                        total={resources.machines.total}
                        suffix="online"
                    />
                    <ResourceBar
                        icon={RiPlayCircleLine}
                        iconColor="text-emerald-400"
                        barColor="bg-emerald-500"
                        label="Runners"
                        current={resources.runners.busy}
                        total={resources.runners.total}
                        suffix="busy"
                    />
                    <ResourceBar
                        icon={RiCpuLine}
                        iconColor="text-purple-400"
                        barColor="bg-purple-500"
                        label="CPU"
                        current={resources.cpu.allocated}
                        total={resources.cpu.limit}
                        suffix="vCPU"
                    />
                    <ResourceBar
                        icon={RiRam2Line}
                        iconColor="text-orange-400"
                        barColor="bg-orange-500"
                        label="Memory"
                        current={resources.ram.allocated}
                        total={resources.ram.limit}
                        suffix="GB"
                    />
                </div>
            </motion.div>

            {/* Charts Row */}
            <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
                {/* Runners Chart */}
                <motion.div
                    className="lg:col-span-2 rounded-xl border border-white/10 bg-white/[0.02]"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.25 }}
                >
                    <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
                        <div>
                            <h3 className="font-mono text-sm text-white">Runner Activity</h3>
                            <p className="mt-0.5 font-mono text-xs text-white/40">Daily runner executions</p>
                        </div>
                        <div className="flex items-center gap-4">
                            <div className="flex items-center gap-1.5">
                                <div className="size-2.5 rounded-sm bg-emerald-500" />
                                <span className="font-mono text-[10px] text-white/50">Completed</span>
                            </div>
                            <div className="flex items-center gap-1.5">
                                <div className="size-2.5 rounded-sm bg-red-500" />
                                <span className="font-mono text-[10px] text-white/50">Cancelled</span>
                            </div>
                        </div>
                    </div>
                    <div className="p-5">
                        <div className="flex items-end gap-1" style={{ height: 140 }}>
                            {chartData.map((day, index) => {
                                const height = maxRunners > 0 ? (day.runners / maxRunners) * 100 : 0;
                                const completedRatio = day.runners > 0 ? day.completed / day.runners : 1;
                                return (
                                    <div
                                        key={`${day.date}-${index}`}
                                        className="group relative flex flex-1 flex-col items-center"
                                    >
                                        <div className="relative w-full" style={{ height: 120 }}>
                                            <motion.div
                                                className="absolute bottom-0 left-1/2 w-full max-w-[32px] -translate-x-1/2 rounded-t-sm bg-red-500/80"
                                                initial={{ height: 0 }}
                                                animate={{ height: `${height}%` }}
                                                transition={{ duration: 0.4, delay: 0.3 + index * 0.02 }}
                                            />
                                            <motion.div
                                                className="absolute bottom-0 left-1/2 w-full max-w-[32px] -translate-x-1/2 rounded-t-sm bg-emerald-500"
                                                initial={{ height: 0 }}
                                                animate={{ height: `${height * completedRatio}%` }}
                                                transition={{ duration: 0.4, delay: 0.35 + index * 0.02 }}
                                            />
                                        </div>
                                        {/* Tooltip */}
                                        <div className="pointer-events-none absolute -top-10 left-1/2 -translate-x-1/2 opacity-0 transition-opacity group-hover:opacity-100">
                                            <div className="whitespace-nowrap rounded bg-white/10 px-2 py-1 font-mono text-[10px] text-white backdrop-blur">
                                                {day.runners} runners ({day.completed} ok)
                                            </div>
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                        <div className="mt-2 flex justify-between">
                            <span className="font-mono text-[10px] text-white/30">{chartData[0]?.date.split(' ')[0]}</span>
                            <span className="font-mono text-[10px] text-white/30">{chartData[chartData.length - 1]?.date.split(' ')[0]}</span>
                        </div>
                    </div>
                </motion.div>

                {/* Runner Status Breakdown */}
                <motion.div
                    className="rounded-xl border border-white/10 bg-white/[0.02]"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.3 }}
                >
                    <div className="border-b border-white/10 px-5 py-4">
                        <h3 className="font-mono text-sm text-white">Runner Status</h3>
                        <p className="mt-0.5 font-mono text-xs text-white/40">Current breakdown</p>
                    </div>
                    <div className="p-5 space-y-4">
                        <StatusRow
                            bgColor="bg-emerald-500"
                            label="Busy"
                            count={statusBreakdown.busy}
                            description="Currently executing"
                        />
                        <StatusRow
                            bgColor="bg-blue-500"
                            label="Idle"
                            count={statusBreakdown.idle}
                            description="Ready for work"
                        />
                        <StatusRow
                            bgColor="bg-amber-500"
                            label="Pending"
                            count={statusBreakdown.pending}
                            description="Starting up"
                        />
                        <StatusRow
                            bgColor="bg-white/30"
                            label="Completed"
                            count={statusBreakdown.completed}
                            description="This period"
                        />
                        <div className="pt-4 border-t border-white/10">
                            <Link
                                to="/$login/runners"
                                params={{ login: selectedInstallation?.login || '' }}
                                className="flex items-center justify-center gap-1 font-mono text-xs text-white/50 transition-colors hover:text-white"
                            >
                                View all runners
                                <RiArrowRightSLine className="size-3.5" />
                            </Link>
                        </div>
                    </div>
                </motion.div>
            </div>
        </div>
    );
}

function StatBox({
    icon: Icon,
    iconColor,
    label,
    value,
    subValue,
    comparison,
    delay = 0,
}: {
    icon: typeof RiPlayCircleLine;
    iconColor: string;
    label: string;
    value: string;
    subValue?: string;
    comparison?: { value: number; label: string; suffix?: string; invertColors?: boolean };
    delay?: number;
}) {
    const isPositive = comparison ? (comparison.invertColors ? comparison.value < 0 : comparison.value > 0) : false;
    const isNegative = comparison ? (comparison.invertColors ? comparison.value > 0 : comparison.value < 0) : false;

    return (
        <motion.div
            className="rounded-xl border border-white/10 bg-white/[0.02] p-5"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, delay }}
        >
            <div className="flex items-center gap-2">
                <Icon className={cx('size-4', iconColor)} />
                <span className="font-mono text-xs text-white/50">{label}</span>
            </div>
            <p className="mt-2 font-display text-2xl text-white">{value}</p>
            {subValue && (
                <p className="mt-1 font-mono text-xs text-white/40">{subValue}</p>
            )}
            {comparison && (
                <div className="mt-2 flex items-center gap-1">
                    {comparison.value !== 0 && (
                        <>
                            {isPositive ? (
                                <RiArrowUpSLine className="size-3.5 text-emerald-400" />
                            ) : isNegative ? (
                                <RiArrowDownSLine className="size-3.5 text-red-400" />
                            ) : null}
                            <span className={cx(
                                'font-mono text-xs',
                                isPositive ? 'text-emerald-400' : isNegative ? 'text-red-400' : 'text-white/40'
                            )}>
                                {Math.abs(comparison.value)}{comparison.suffix || '%'}
                            </span>
                        </>
                    )}
                    <span className="font-mono text-[10px] text-white/30">{comparison.label}</span>
                </div>
            )}
        </motion.div>
    );
}

function ResourceBar({
    icon: Icon,
    iconColor,
    barColor,
    label,
    current,
    total,
    suffix,
}: {
    icon: typeof RiServerLine;
    iconColor: string;
    barColor: string;
    label: string;
    current: number;
    total: number;
    suffix: string;
}) {
    const percent = total > 0 ? (current / total) * 100 : 0;

    return (
        <div>
            <div className="mb-1.5 flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                    <Icon className={cx('size-3.5', iconColor)} />
                    <span className="font-mono text-xs text-white/70">{label}</span>
                </div>
                <span className="font-mono text-xs text-white">
                    {current}<span className="text-white/40">/{total}</span> <span className="text-white/40">{suffix}</span>
                </span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
                <motion.div
                    className={cx('h-full rounded-full', barColor)}
                    initial={{ width: 0 }}
                    animate={{ width: `${percent}%` }}
                    transition={{ duration: 0.5, delay: 0.3 }}
                />
            </div>
        </div>
    );
}

function StatusRow({
    bgColor,
    label,
    count,
    description,
}: {
    bgColor: string;
    label: string;
    count: number;
    description: string;
}) {
    return (
        <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
                <div className={cx('size-3 rounded-sm', bgColor)} />
                <div>
                    <span className="font-mono text-xs text-white/70">{label}</span>
                    <p className="font-mono text-[10px] text-white/30">{description}</p>
                </div>
            </div>
            <span className="font-mono text-sm text-white">{count}</span>
        </div>
    );
}
