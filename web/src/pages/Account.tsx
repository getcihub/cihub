import { UserEmails } from '@/components/UserEmails';
import { useAuth } from '@/hooks/useAuth';
import { useInstallations } from '@/hooks/useInstallations';
import { useUser } from '@/hooks/useUser';
import { RiRefreshLine } from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
import { motion } from 'framer-motion';
import { useEffect, useState } from 'react';
import { toast } from 'sonner';

export function AccountPage() {
    const navigate = useNavigate();
    const { user, logout } = useAuth();
    const { data: userData, refetch: refetchUser } = useUser();
    const { refetch: refetchInstallations } = useInstallations();
    const [isSyncing, setIsSyncing] = useState(false);

    const handleLogout = async () => {
        await logout();
        navigate({ to: '/login' });
    };

    // Poll user sync status and refresh installations when sync completes
    useEffect(() => {
        if (!userData?.syncing) {
            return;
        }

        const interval = setInterval(async () => {
            const result = await refetchUser();
            // If user is no longer syncing, refetch installations
            if (!result.data?.syncing) {
                await refetchInstallations();
            }
        }, 2000); // Check every 2 seconds

        return () => clearInterval(interval);
    }, [userData?.syncing, refetchUser, refetchInstallations]);

    const handleSyncInstallations = async () => {
        setIsSyncing(true);

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
                setIsSyncing(false);
            }
        };

        toast.promise(syncPromise(), {
            loading: 'Syncing your installations...',
            success: () => 'Installations synchronized successfully',
            error: 'Failed to synchronize installations. Please try again.',
        });
    };

    if (!user) {
        return (
            <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507]">
                <p className="font-mono text-sm text-white/50">Loading...</p>
            </div>
        );
    }

    return (
        <div className="grid-bg min-h-screen bg-[#050507] px-4 py-10 sm:px-6">
            <motion.div
                className="mx-auto max-w-2xl"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                <div className="mb-8">
                    <h1 className="font-display text-3xl text-white">
                        Account Settings
                    </h1>
                    <p className="mt-2 font-mono text-sm text-white/50">
                        Manage your account and preferences
                    </p>
                </div>

                {/* Profile Section */}
                <motion.div
                    className="mb-6 rounded-xl border border-white/10 bg-white/[0.02] p-6"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.1 }}
                >
                    <h2 className="mb-6 font-display text-lg text-white">
                        Profile
                    </h2>
                    <div className="mb-8 flex items-center gap-6">
                        <img
                            src={user.avatar_url}
                            alt={user.login}
                            className="size-16 rounded-full border-2 border-white/10 object-cover"
                        />
                        <div>
                            <h3 className="font-mono text-base text-white">
                                {user.login}
                            </h3>
                            <p className="mt-1 font-mono text-sm text-white/50">
                                {user.email}
                            </p>
                            <p className="mt-2 font-mono text-xs text-white/30">
                                Member since{' '}
                                {new Date(
                                    user.created_at * 1000,
                                ).toLocaleDateString()}
                            </p>
                        </div>
                    </div>
                </motion.div>

                {/* Email Preferences Section */}
                <UserEmails />

                {/* Installations Sync Section */}
                <motion.div
                    className="mb-6 rounded-xl border border-white/10 bg-white/[0.02] p-6"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.2 }}
                >
                    <h2 className="mb-4 font-display text-lg text-white">
                        Installations
                    </h2>
                    <p className="mb-6 font-mono text-xs text-white/50">
                        Synchronize your GitHub installations to ensure they're up
                        to date. This fetches all installations you have access to.
                    </p>
                    <div className="flex items-center gap-4">
                        <button
                            onClick={handleSyncInstallations}
                            disabled={isSyncing || userData?.syncing}
                            className="inline-flex items-center gap-2 rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            <RiRefreshLine
                                className={`size-4 ${isSyncing || userData?.syncing ? 'animate-spin' : ''}`}
                            />
                            {isSyncing || userData?.syncing
                                ? 'Syncing...'
                                : 'Sync Installations'}
                        </button>
                        {userData?.synced_at ? (
                            <p className="font-mono text-xs text-white/30">
                                Last synced:{' '}
                                {new Date(
                                    userData.synced_at * 1000,
                                ).toLocaleString()}
                            </p>
                        ) : null}
                    </div>
                </motion.div>

                {/* Logout Section */}
                <motion.div
                    className="rounded-xl border border-red-500/20 bg-red-500/5 p-6"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.3 }}
                >
                    <h2 className="mb-4 font-display text-lg text-white">
                        Session
                    </h2>
                    <p className="mb-4 font-mono text-xs text-white/50">
                        Sign out from your account. You'll need to authenticate
                        again to access this app.
                    </p>
                    <button
                        onClick={handleLogout}
                        className="inline-flex items-center rounded-md bg-red-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-red-400"
                    >
                        Sign Out
                    </button>
                </motion.div>
            </motion.div>
        </div>
    );
}
