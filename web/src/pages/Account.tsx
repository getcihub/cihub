import { useAuth } from '@/hooks/useAuth';
import { useEmails } from '@/hooks/useEmails';
import { useInstallations } from '@/hooks/useInstallations';
import { useUpdateEmail } from '@/hooks/useUpdateEmail';
import { useUser } from '@/hooks/useUser';
import { cx } from '@/lib/utils';
import type { UserEmail } from '@/types/user';
import {
    RiArrowLeftLine,
    RiCalendarLine,
    RiCheckLine,
    RiGithubFill,
    RiLogoutBoxRLine,
    RiMailLine,
    RiRefreshLine,
    RiShieldCheckLine,
    RiUserLine,
} from '@remixicon/react';
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
            if (!result.data?.syncing) {
                await refetchInstallations();
            }
        }, 2000);

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
                await refetchInstallations();
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

    const formatDate = (timestamp: number) => {
        return new Date(timestamp * 1000).toLocaleDateString('en-US', {
            year: 'numeric',
            month: 'long',
            day: 'numeric',
        });
    };

    if (!user) {
        return (
            <div className="flex min-h-screen items-center justify-center">
                <div className="size-8 animate-spin rounded-full border-2 border-white/20 border-t-white" />
            </div>
        );
    }

    return (
        <motion.div
            className="mx-auto max-w-3xl space-y-8 px-4 py-8 sm:px-6"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
        >
            {/* Back Button */}
            <button
                onClick={() => navigate({ to: '/' })}
                className="inline-flex items-center gap-2 font-mono text-sm text-white/60 transition-colors hover:text-white"
            >
                <RiArrowLeftLine className="size-4" />
                Back to Dashboard
            </button>

            {/* Header */}
            <div>
                <h1 className="font-display text-3xl text-white">Account Settings</h1>
                <p className="mt-2 font-mono text-sm text-white/50">
                    Manage your profile, email preferences, and account settings
                </p>
            </div>

            {/* Profile Card */}
            <motion.div
                className="overflow-hidden rounded-xl border border-white/10 bg-white/[0.02]"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.1 }}
            >
                <div className="border-b border-white/10 px-6 py-4">
                    <div className="flex items-center gap-2">
                        <RiUserLine className="size-5 text-white/50" />
                        <h2 className="font-display text-lg text-white">Profile</h2>
                    </div>
                </div>
                <div className="p-6">
                    <div className="flex items-start gap-6">
                        <img
                            src={user.avatar_url}
                            alt={user.login}
                            className="size-20 rounded-full border-2 border-white/10 object-cover"
                        />
                        <div className="flex-1">
                            <div className="flex items-center gap-3">
                                <h3 className="font-display text-xl text-white">
                                    {user.login}
                                </h3>
                                <a
                                    href={`https://github.com/${user.login}`}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    className="inline-flex items-center gap-1 rounded-full bg-white/5 px-2.5 py-1 font-mono text-xs text-white/50 transition-colors hover:bg-white/10 hover:text-white"
                                >
                                    <RiGithubFill className="size-3.5" />
                                    GitHub
                                </a>
                            </div>
                            <p className="mt-1 font-mono text-sm text-white/60">
                                {user.email}
                            </p>
                            <div className="mt-4 flex items-center gap-4">
                                <div className="flex items-center gap-1.5 font-mono text-xs text-white/40">
                                    <RiCalendarLine className="size-3.5" />
                                    Member since {formatDate(user.created_at)}
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </motion.div>

            {/* Email Preferences Card */}
            <EmailPreferencesCard />

            {/* Installations Card */}
            <motion.div
                className="overflow-hidden rounded-xl border border-white/10 bg-white/[0.02]"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.25 }}
            >
                <div className="border-b border-white/10 px-6 py-4">
                    <div className="flex items-center gap-2">
                        <RiRefreshLine className="size-5 text-white/50" />
                        <h2 className="font-display text-lg text-white">
                            GitHub Installations
                        </h2>
                    </div>
                </div>
                <div className="p-6">
                    <p className="mb-6 font-mono text-sm text-white/50">
                        Synchronize your GitHub App installations to ensure they're up to
                        date. This fetches all organizations and repositories you have
                        access to.
                    </p>
                    <div className="flex flex-wrap items-center gap-4">
                        <button
                            onClick={handleSyncInstallations}
                            disabled={isSyncing || userData?.syncing}
                            className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.02] px-4 py-2.5 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            <RiRefreshLine
                                className={cx(
                                    'size-4',
                                    (isSyncing || userData?.syncing) && 'animate-spin',
                                )}
                            />
                            {isSyncing || userData?.syncing
                                ? 'Syncing...'
                                : 'Sync Installations'}
                        </button>
                        {userData?.synced_at && (
                            <span className="font-mono text-xs text-white/40">
                                Last synced:{' '}
                                {new Date(userData.synced_at * 1000).toLocaleString()}
                            </span>
                        )}
                    </div>
                </div>
            </motion.div>

            {/* Danger Zone */}
            <motion.div
                className="overflow-hidden rounded-xl border border-red-500/20 bg-red-500/5"
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3, delay: 0.3 }}
            >
                <div className="border-b border-red-500/20 px-6 py-4">
                    <div className="flex items-center gap-2">
                        <RiLogoutBoxRLine className="size-5 text-red-400/70" />
                        <h2 className="font-display text-lg text-white">Session</h2>
                    </div>
                </div>
                <div className="p-6">
                    <p className="mb-6 font-mono text-sm text-white/50">
                        Sign out from your account. You'll need to authenticate again with
                        GitHub to access CIHub.
                    </p>
                    <button
                        onClick={handleLogout}
                        className="inline-flex items-center gap-2 rounded-lg bg-red-500 px-4 py-2.5 font-mono text-sm text-white transition-colors hover:bg-red-400"
                    >
                        <RiLogoutBoxRLine className="size-4" />
                        Sign Out
                    </button>
                </div>
            </motion.div>
        </motion.div>
    );
}

function EmailPreferencesCard() {
    const { user } = useAuth();
    const { data: emails = [], isLoading } = useEmails();
    const { mutate: updateEmail, isPending, isSuccess } = useUpdateEmail();

    const [selectedEmail, setSelectedEmail] = useState('');
    const [originalEmail, setOriginalEmail] = useState('');
    const [isOpen, setIsOpen] = useState(false);

    const hasChanges = selectedEmail !== originalEmail && selectedEmail !== '';

    useEffect(() => {
        if (user?.email && originalEmail === '') {
            setSelectedEmail(user.email);
            setOriginalEmail(user.email);
        } else if (emails.length > 0 && originalEmail === '' && !user?.email) {
            setSelectedEmail(emails[0].email);
            setOriginalEmail(emails[0].email);
        }
    }, [user?.email, emails, originalEmail]);

    useEffect(() => {
        if (isSuccess && selectedEmail) {
            setOriginalEmail(selectedEmail);
            toast.success(`Email updated to ${selectedEmail}`);
        }
    }, [isSuccess, selectedEmail]);

    const handleSave = () => {
        if (hasChanges) {
            updateEmail({ email: selectedEmail });
        }
    };

    const selectedEmailData = emails.find((e) => e.email === selectedEmail);

    return (
        <motion.div
            className="overflow-hidden rounded-xl border border-white/10 bg-white/[0.02]"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.3, delay: 0.2 }}
        >
            <div className="border-b border-white/10 px-6 py-4">
                <div className="flex items-center gap-2">
                    <RiMailLine className="size-5 text-white/50" />
                    <h2 className="font-display text-lg text-white">Email Address</h2>
                </div>
            </div>
            <div className="p-6">
                <p className="mb-6 font-mono text-sm text-white/50">
                    Email address used for account-related notifications and alerts.
                </p>

                {isLoading ? (
                    <div className="h-12 w-full animate-pulse rounded-lg bg-white/5" />
                ) : (
                    <div className="space-y-4">
                        {/* Custom Select */}
                        <div className="relative">
                            <button
                                type="button"
                                onClick={() => setIsOpen(!isOpen)}
                                className="flex w-full items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] px-4 py-3 text-left transition-colors hover:border-white/20 focus:border-white/30 focus:outline-none"
                            >
                                <div className="flex items-center gap-3">
                                    <span className="font-mono text-sm text-white">
                                        {selectedEmail || 'Select an email'}
                                    </span>
                                    {selectedEmailData?.verified && (
                                        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-400">
                                            <RiShieldCheckLine className="size-3" />
                                            verified
                                        </span>
                                    )}
                                    {selectedEmailData?.primary && (
                                        <span className="rounded-full bg-blue-500/10 px-2 py-0.5 text-xs text-blue-400">
                                            primary
                                        </span>
                                    )}
                                </div>
                                <svg
                                    className={cx(
                                        'size-4 text-white/40 transition-transform',
                                        isOpen && 'rotate-180',
                                    )}
                                    fill="none"
                                    viewBox="0 0 24 24"
                                    stroke="currentColor"
                                >
                                    <path
                                        strokeLinecap="round"
                                        strokeLinejoin="round"
                                        strokeWidth={2}
                                        d="M19 9l-7 7-7-7"
                                    />
                                </svg>
                            </button>

                            {isOpen && (
                                <div className="absolute z-10 mt-2 w-full overflow-hidden rounded-lg border border-white/10 bg-[#0a0a0c] shadow-xl">
                                    {emails.map((email: UserEmail) => (
                                        <button
                                            key={email.email}
                                            type="button"
                                            onClick={() => {
                                                setSelectedEmail(email.email);
                                                setIsOpen(false);
                                            }}
                                            className={cx(
                                                'flex w-full items-center justify-between px-4 py-3 text-left transition-colors hover:bg-white/5',
                                                selectedEmail === email.email &&
                                                    'bg-white/5',
                                            )}
                                        >
                                            <div className="flex items-center gap-3">
                                                <span className="font-mono text-sm text-white">
                                                    {email.email}
                                                </span>
                                                {email.verified && (
                                                    <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-400">
                                                        <RiShieldCheckLine className="size-3" />
                                                        verified
                                                    </span>
                                                )}
                                                {email.primary && (
                                                    <span className="rounded-full bg-blue-500/10 px-2 py-0.5 text-xs text-blue-400">
                                                        primary
                                                    </span>
                                                )}
                                            </div>
                                            {selectedEmail === email.email && (
                                                <RiCheckLine className="size-4 text-white" />
                                            )}
                                        </button>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>
                )}
            </div>

            {/* Footer */}
            <div className="flex items-center justify-between border-t border-white/10 bg-white/[0.01] px-6 py-4">
                <span className="font-mono text-xs text-white/40">
                    {hasChanges
                        ? 'You have unsaved changes'
                        : 'Select an email to receive notifications'}
                </span>
                <button
                    onClick={handleSave}
                    disabled={!hasChanges || isPending || isLoading}
                    className={cx(
                        'inline-flex items-center gap-2 rounded-lg px-4 py-2 font-mono text-sm transition-all',
                        hasChanges
                            ? 'bg-white text-black hover:bg-white/90'
                            : 'cursor-not-allowed bg-white/10 text-white/40',
                    )}
                >
                    {isPending ? (
                        <>
                            <div className="size-4 animate-spin rounded-full border-2 border-black/20 border-t-black" />
                            Saving...
                        </>
                    ) : (
                        'Save Changes'
                    )}
                </button>
            </div>
        </motion.div>
    );
}
