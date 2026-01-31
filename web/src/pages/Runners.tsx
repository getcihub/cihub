import { Suspense, useMemo } from 'react';
import { useParams } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { motion } from 'framer-motion';
import { RiCpuLine, RiDatabase2Line, RiServerLine } from '@remixicon/react';
import { AppHeader } from '@/components/AppHeader';
import { PageContainer } from '@/components/PageContainer';
import { Card } from '@/components/Card';
import { EmptyState, LoadingState } from '@/components/PageState';
import { ProgressBar } from '@/components/ProgressBar';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/Table';
import { RunnerStatusBadge } from '@/components/RunnerStatusBadge';
import { formatBytes, formatTimestamp } from '@/lib/utils';
import type { APIResponse, Runner } from '@/types/api';

const MIB_TO_BYTES = 1024 * 1024;
const formatMib = (mib: number) => formatBytes(mib * MIB_TO_BYTES);

function RunnersPageContent() {
  const { owner } = useParams({ from: '/installations/$owner/runners' });

  const {
    data: response,
    isLoading,
    error,
    refetch,
  } = useQuery<APIResponse<Runner[]>>({
    queryKey: ['runners', owner],
    queryFn: async () => {
      const res = await fetch(`/api/installations/${owner}/runners`);
      if (!res.ok) throw new Error('Failed to fetch runners');
      return res.json();
    },
    enabled: !!owner,
    refetchInterval: 5000,
  });

  const runners = response?.data || [];
  const busyRunners = useMemo(() => runners.filter((runner) => runner.status === 'busy'), [runners]);

  const stats = useMemo(() => {
    return {
      total: runners.length,
      busy: busyRunners.length,
      busyCPU: busyRunners.reduce((sum, r) => sum + r.cpu, 0),
      totalCPU: runners.reduce((sum, r) => sum + r.cpu, 0),
      busyRAM: busyRunners.reduce((sum, r) => sum + r.ram, 0),
      totalRAM: runners.reduce((sum, r) => sum + r.ram, 0),
    };
  }, [runners, busyRunners]);

  if (!owner) {
    return (
      <div className="min-h-screen bg-[#050507] text-white grid-bg">
        <PageContainer className="py-16">
          <EmptyState
            icon={RiServerLine}
            title="No installation selected"
            description="Please select an installation to view runners."
          />
        </PageContainer>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-[#050507] text-white grid-bg">
      <AppHeader />

      <PageContainer className="py-8">
        <div className="mb-8 flex flex-col gap-4">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div>
              <p className="font-mono text-[10px] uppercase tracking-[0.2em] text-white/40">
                Activity
              </p>
              <h1 className="mt-2 font-display text-3xl text-white">Runners</h1>
              <p className="mt-2 font-mono text-xs text-muted">
                Busy runners currently executing jobs for {owner}
              </p>
            </div>
          </div>
        </div>

        {isLoading ? (
          <LoadingState />
        ) : error ? (
          <EmptyState
            icon={RiServerLine}
            title="Failed to load runners"
            description="There was an error loading the runners list. Please try again."
            action={
              <button
                onClick={() => refetch()}
                className="px-3 py-1.5 rounded font-mono text-xs bg-white/5 text-white/60 border border-white/10 hover:bg-white/10 hover:text-white/80"
              >
                Retry
              </button>
            }
          />
        ) : busyRunners.length === 0 ? (
          <EmptyState
            icon={RiServerLine}
            title="No busy runners"
            description="There are no runners executing jobs right now."
          />
        ) : (
          <div className="space-y-6">
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
                      Busy runners
                    </div>
                    <div className="text-3xl font-display font-light tracking-tight text-white">
                      {stats.busy}
                    </div>
                    <div className="text-xs font-mono text-muted mt-2">
                      {stats.total} total
                    </div>
                  </div>
                  <div className="rounded-lg bg-amber-500/10 p-2">
                    <RiServerLine className="size-5 text-amber-400" />
                  </div>
                </div>
              </Card>

              <Card padding="md" animate delay={0.15} className="relative overflow-hidden">
                <div className="absolute left-0 top-0 h-full w-1 bg-cyan-500/70" />
                <div className="flex items-start justify-between">
                  <div>
                    <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                      CPU in use
                    </div>
                    <div className="text-3xl font-display font-light tracking-tight text-white">
                      {stats.busyCPU}/{stats.totalCPU}
                    </div>
                    <div className="text-xs font-mono text-muted mt-2">
                      busy vs total
                    </div>
                  </div>
                  <div className="rounded-lg bg-cyan-500/10 p-2">
                    <RiCpuLine className="size-5 text-cyan-400" />
                  </div>
                </div>
                <div className="mt-3">
                  <ProgressBar value={stats.busyCPU} max={stats.totalCPU} color="#06b6d4" />
                </div>
              </Card>

              <Card padding="md" animate delay={0.25} className="relative overflow-hidden">
                <div className="absolute left-0 top-0 h-full w-1 bg-purple-500/70" />
                <div className="flex items-start justify-between">
                  <div>
                    <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                      RAM in use
                    </div>
                    <div className="text-3xl font-display font-light tracking-tight text-white">
                      {formatMib(stats.busyRAM)}
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
                  <ProgressBar value={stats.busyRAM} max={stats.totalRAM} color="#a855f7" />
                </div>
              </Card>
            </motion.div>

            <motion.div
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.5, delay: 0.2 }}
            >
              <Card padding="none" className="overflow-hidden">
                <div className="overflow-x-auto">
                  <table className="w-full min-w-[900px]">
                    <thead>
                      <tr className="border-b border-white/10 bg-white/[0.02]">
                        <TableHead>Status</TableHead>
                        <TableHead>Runner</TableHead>
                        <TableHead>Machine</TableHead>
                        <TableHead className="text-right">CPU</TableHead>
                        <TableHead className="text-right">RAM</TableHead>
                        <TableHead>Labels</TableHead>
                        <TableHead className="text-right">Started</TableHead>
                      </tr>
                    </thead>
                    <TableBody>
                      {busyRunners.map((runner, idx) => (
                        <motion.tr
                          key={runner.name}
                          initial={{ opacity: 0 }}
                          animate={{ opacity: 1 }}
                          transition={{ delay: 0.05 + idx * 0.03 }}
                          className="group border-b border-white/5 hover:bg-white/[0.02] transition-colors"
                        >
                          <TableCell>
                            <RunnerStatusBadge status={runner.status} cancelled={runner.cancelled > 0} />
                          </TableCell>
                          <TableCell className="font-mono text-sm text-white">{runner.name}</TableCell>
                          <TableCell className="font-mono text-xs text-white/70">{runner.machine}</TableCell>
                          <TableCell className="font-mono text-xs text-white/70 text-right">
                            {runner.cpu}
                          </TableCell>
                          <TableCell className="font-mono text-xs text-white/70 text-right">
                            {formatMib(runner.ram)}
                          </TableCell>
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
                          <TableCell className="text-right font-mono text-xs text-white/50">
                            {runner.started ? formatTimestamp(runner.started) : '—'}
                          </TableCell>
                        </motion.tr>
                      ))}
                    </TableBody>
                  </table>
                </div>
              </Card>
            </motion.div>
          </div>
        )}
      </PageContainer>
    </div>
  );
}

export default function RunnersPage() {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen bg-[#050507] text-white grid-bg flex items-center justify-center">
          <div className="font-mono text-sm text-white/40">Loading...</div>
        </div>
      }
    >
      <RunnersPageContent />
    </Suspense>
  );
}
