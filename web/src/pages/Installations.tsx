import { useInstallation } from '@/hooks/useInstallation';
import { useInstallations } from '@/hooks/useInstallations';
import { useUser } from '@/hooks/useUser';
import { useVarz } from '@/hooks/useVarz';
import { RiAddLargeLine, RiAddLine, RiArrowRightSLine } from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useEffect, useState } from 'react';
import { toast } from 'sonner';

export function InstallationsPage() {
    const navigate = useNavigate();
    const [isSyncingRequest, setIsSyncingRequest] = useState(false);
    const { data: user, refetch: refetchUser } = useUser();
    const {
        data: installations = [],
        isLoading,
        error,
        refetch: refetchInstallations,
    } = useInstallations();
    const {
        selectInstallation,
        selectedInstallation,
        isLoading: installationLoading,
    } = useInstallation();
    const { data: varz } = useVarz();

    // Poll user sync status and refresh installations when sync completes
    useEffect(() => {
        if (!user?.syncing) {
            return;
        }

        // Reset request loading state once backend confirms syncing
        setIsSyncingRequest(false);

        const interval = setInterval(async () => {
            const result = await refetchUser();
            // If user is no longer syncing, refetch installations
            if (!result.data?.syncing) {
                await refetchInstallations();
            }
        }, 2000); // Check every 2 seconds

        return () => clearInterval(interval);
    }, [user?.syncing, refetchUser, refetchInstallations]);

    // Auto-redirect to selected installation if available
    useEffect(() => {
        if (!installationLoading && selectedInstallation) {
            navigate({
                to: '/$login/machines',
                params: { login: selectedInstallation.login },
            });
        }
    }, [selectedInstallation, installationLoading, navigate]);

    const handleSelectInstallation = async (installationId: number) => {
        const installation = installations.find((i) => i.id === installationId);
        if (!installation) return;

        try {
            await selectInstallation(installation);
            navigate({
                to: '/$login/machines',
                params: { login: installation.login },
            });
        } catch (err) {
            console.error('Failed to select installation:', err);
        }
    };

    const handleAddInstallation = () => {
        if (varz?.github?.name) {
            // Redirect to GitHub App installation page
            window.location.href = `https://github.com/apps/${varz.github.name}/installations/new`;
        } else {
            // Fallback to auth install if app name is not available
            window.location.href = '/auth/install';
        }
    };

    const handleSyncInstallations = async () => {
        setIsSyncingRequest(true);

        const syncPromise = async () => {
            try {
                const response = await fetch('/api/user/installations', {
                    method: 'POST',
                });
                if (!response.ok) {
                    throw new Error('Failed to sync installations');
                }
                // Refetch installations immediately in case API returned them
                await refetchInstallations();
                // Refetch user data to update syncing status
                await refetchUser();

                return { success: true };
            } catch (err) {
                console.error('Failed to sync installations:', err);
                throw err;
            } finally {
                setIsSyncingRequest(false);
            }
        };

        toast.promise(syncPromise(), {
            loading: 'Syncing your installations...',
            success: () => 'Installations synchronized successfully',
            error: 'Failed to synchronize installations. Please try again.',
        });
    };

    // Loading state
    if (isLoading) {
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

    // Error state
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

    // Empty state - syncing
    if (installations.length === 0 && user?.syncing) {
        return (
            <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507] px-4">
                <div className="w-full max-w-md">
                    <h1 className="mb-8 text-center font-display text-2xl text-white">
                        Select an Installation
                    </h1>
                    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-8">
                        <div className="text-center">
                            <div className="mx-auto mb-4 w-fit">
                                <div className="inline-block h-8 w-8 animate-spin rounded-full border-4 border-white/10 border-t-amber-500" />
                            </div>
                            <h3 className="mb-2 font-display text-lg text-white">
                                Syncing Your Data
                            </h3>
                            <p className="font-mono text-xs text-white/50">
                                We're fetching your installation data. Please wait a moment.
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        );
    }

    // Empty state - no installations
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
                            <div className="flex flex-col gap-3">
                                <button
                                    onClick={handleAddInstallation}
                                    className="inline-flex w-full items-center justify-center gap-2 rounded-md bg-amber-500 px-4 py-2.5 font-mono text-sm text-white transition-all hover:bg-amber-400"
                                >
                                    <RiAddLine className="size-4" />
                                    Add Installation
                                </button>
                                <button
                                    onClick={handleSyncInstallations}
                                    disabled={user?.syncing || isSyncingRequest}
                                    className="inline-flex w-full items-center justify-center rounded-md border border-white/10 bg-white/[0.02] px-4 py-2.5 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                                >
                                    {user?.syncing || isSyncingRequest
                                        ? 'Syncing...'
                                        : "Can't see your installation? Synchronize"}
                                </button>
                            </div>
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
                                onClick={() => handleSelectInstallation(installation.id)}
                                className="group w-full text-left"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: index * 0.05 }}
                            >
                                <div className="flex items-center gap-4 rounded-xl border border-white/10 bg-white/[0.02] p-4 transition-all hover:border-white/20 hover:bg-white/[0.04]">
                                    {installation.avatar_url ? (
                                        <img
                                            src={installation.avatar_url}
                                            alt={installation.login}
                                            className="size-12 flex-shrink-0 rounded-lg border border-white/10 object-cover"
                                        />
                                    ) : (
                                        <div className="flex size-12 flex-shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-amber-400 to-amber-600 font-display text-lg font-semibold text-white">
                                            {installation.login.charAt(0).toUpperCase()}
                                        </div>
                                    )}
                                    <div className="min-w-0 flex-1">
                                        <h2 className="truncate font-mono text-sm text-white">
                                            {installation.login}
                                        </h2>
                                    </div>
                                    <div className="flex-shrink-0">
                                        <RiArrowRightSLine className="size-5 text-white/30 transition-all group-hover:translate-x-0.5 group-hover:text-white/50" />
                                    </div>
                                </div>
                            </motion.button>
                        ))}
                    </div>

                    {/* Sync Section */}
                    {user?.syncing ? (
                        <div className="py-6 text-center">
                            <div className="mb-4 inline-block h-8 w-8 animate-spin rounded-full border-4 border-white/10 border-t-amber-500" />
                            <p className="font-mono text-xs text-white/50">
                                Syncing your installations...
                            </p>
                        </div>
                    ) : (
                        <div className="flex flex-col gap-3">
                            <button
                                onClick={handleSyncInstallations}
                                disabled={user?.syncing || isSyncingRequest}
                                className="inline-flex w-full items-center justify-center rounded-md border border-white/10 bg-white/[0.02] px-4 py-2.5 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                            >
                                {user?.syncing || isSyncingRequest
                                    ? 'Syncing...'
                                    : "Can't see your installation? Synchronize"}
                            </button>
                            <button
                                onClick={handleAddInstallation}
                                className="inline-flex w-full items-center justify-center gap-2 rounded-md bg-amber-500 px-4 py-2.5 font-mono text-sm text-white transition-all hover:bg-amber-400"
                            >
                                <RiAddLine className="size-4" />
                                Add Installation
                            </button>
                        </div>
                    )}
                </div>
            </motion.div>
        </div>
    );
}
