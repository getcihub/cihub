import { useCallback, useState, useEffect, Suspense } from "react";
import { useNavigate } from "@tanstack/react-router";
import { motion } from 'framer-motion';
import { RiAddLargeLine, RiArrowRightSLine } from '@remixicon/react';

interface APIResponse<T> {
    data: T;
    error: boolean;
    reason: boolean;
}

interface Installation {
    avatar: string;
    id: number;
    name: string;
}

function InstallationsPageContent() {
    const navigate = useNavigate();
    const [installations, setInstallations] = useState<Installation[]>([])
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);

    const fetchData = useCallback(async () => {
        setLoading(true);
        setError(null);

        try {
            const res = await fetch(`/api/user/installations`)
            if (!res.ok) {
                throw new Error('Failed to fetch user installations')
            }

            const data = await res.json() as APIResponse<Installation[]>;
            setInstallations(data.data)
        } catch (err) {
            const message = err instanceof Error ? err.message : "Failed to fetch data";
            setError(message);
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => {
        fetchData();
    }, [fetchData]);

    if (loading) {
        return (
            <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507] px-4">
                <div className="w-full max-w-md">
                    <h1 className="mb-8 text-center font-display text-2xl text-white">
                        Select an Installation
                    </h1>
                    <div className="space-y-3">
                        {[...Array(3)].map((_, i) => (
                            <div
                                key={i}
                                className="h-20 animate-pulse rounded-xl bg-white/[0.02] ring-1 ring-white/5"
                            />
                        ))}
                    </div>
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507] px-4">
                <div className="w-full max-w-md">
                    <h1 className="mb-8 text-center font-display text-2xl text-white">
                        Select an Installation
                    </h1>
                    <div className="rounded-xl border border-red-500/20 bg-red-500/10 p-6">
                        <p className="text-center font-mono text-sm text-red-400">
                            Failed to load installations. Please try again later.
                        </p>
                    </div>
                </div>
            </div>
        );
    }

    if (installations.length === 0) {
        return (
            <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507] px-4">
                <motion.div
                    className="w-full max-w-md"
                    initial={{ opacity: 0, y: 20 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.5 }}
                >
                    <h1 className="mb-8 text-center font-display text-2xl text-white">
                        Select an Installation
                    </h1>
                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8">
                        <div className="text-center">
                            <div className="mx-auto mb-4 w-fit rounded-lg bg-white/5 p-3">
                                <RiAddLargeLine className="size-5 text-white/40" />
                            </div>
                            <h3 className="mb-2 font-display text-lg text-white">
                                No Installations Yet
                            </h3>
                            <p className="mb-1 font-mono text-xs text-white/50">
                                You don't have access to any installations yet.
                            </p>
                            <p className="mb-6 font-mono text-[11px] text-white/30">
                                Add a new installation to get started, or contact your
                                organization administrator to grant you access.
                            </p>
                        </div>
                    </div>
                </motion.div>
            </div>
        );
    }

    // Main view with installations
    return (
        <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507] px-4">
            {/* Ambient glow effect */}
            <div
                className="pointer-events-none fixed left-1/2 top-1/2 h-[600px] w-[600px] -translate-x-1/2 -translate-y-1/2 opacity-[0.03]"
                style={{
                    background: 'radial-gradient(circle, #ffffff 0%, transparent 70%)',
                }}
            />

            <motion.div
                className="relative z-10 w-full max-w-md"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                <h1 className="mb-8 text-center font-display text-2xl text-white">
                    Select an Installation
                </h1>

                <div className="space-y-6">
                    {/* Installations List */}
                    <div className="space-y-3">
                        {installations.map((installation, index) => (
                            <motion.button
                                key={installation.id}
                                onClick={() => navigate({ to: '/installations/$owner/machines', params: { owner: installation.name } })}
                                className="group w-full text-left"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: index * 0.05 }}
                            >
                                <div className="flex items-center gap-4 rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-all hover:border-white/20 hover:bg-white/[0.04]">
                                    <img
                                        src={installation.avatar}
                                        alt={installation.name}
                                        className="size-12 flex-shrink-0 rounded-lg border border-white/10 object-cover"
                                    />
                                    <div className="min-w-0 flex-1">
                                        <h2 className="truncate font-mono text-sm text-white">
                                            {installation.name}
                                        </h2>
                                    </div>
                                    <div className="flex-shrink-0">
                                        <RiArrowRightSLine className="size-5 text-white/30 transition-all group-hover:translate-x-0.5 group-hover:text-white/50" />
                                    </div>
                                </div>
                            </motion.button>
                        ))}
                    </div>
                </div>
            </motion.div>
        </div>
    );
}

export default function InstallationsPage() {
    return (
        <Suspense fallback={
            <div className="min-h-screen bg-[#050507] text-white grid-bg flex items-center justify-center">
                <div className="font-mono text-sm text-white/40">Loading...</div>
            </div>
        }>
            <InstallationsPageContent />
        </Suspense>
    );
}
