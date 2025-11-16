import { Button } from '@/components/Button';
import { UserEmails } from '@/components/UserEmails';
import { Card } from '@/components/ui/card';
import { useAuth } from '@/hooks/useAuth';
import { useInstallations } from '@/hooks/useInstallations';
import { useUser } from '@/hooks/useUser';
import { RiRefreshLine } from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
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
            <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                <p className="text-gray-600">Loading...</p>
            </div>
        );
    }

    return (
        <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
            <div className="mb-8">
                <h1 className="text-3xl font-bold text-gray-900">
                    Account Settings
                </h1>
                <p className="text-gray-600 mt-2">
                    Manage your account and preferences
                </p>
            </div>

            {/* Profile Section */}
            <Card className="p-6 mb-6">
                <h2 className="text-xl font-semibold text-gray-900 mb-6">
                    Profile
                </h2>
                <div className="flex items-center gap-6 mb-8">
                    <img
                        src={user.avatar_url}
                        alt={user.login}
                        className="size-16 rounded-full border-2 border-gray-200 object-cover"
                    />
                    <div>
                        <h3 className="text-lg font-medium text-gray-900">
                            {user.login}
                        </h3>
                        <p className="text-sm text-gray-500 mt-1">
                            {user.email}
                        </p>
                        <p className="text-xs text-gray-400 mt-2">
                            Member since{' '}
                            {new Date(
                                user.created_at * 1000,
                            ).toLocaleDateString()}
                        </p>
                    </div>
                </div>
            </Card>

            {/* Email Preferences Section */}
            <UserEmails />

            {/* Installations Sync Section */}
            <Card className="p-6 mb-6">
                <h2 className="text-xl font-semibold text-gray-900 mb-4">
                    Installations
                </h2>
                <p className="text-gray-600 text-sm mb-6">
                    Synchronize your GitHub installations to ensure they're up
                    to date. This fetches all installations you have access to.
                </p>
                <div className="flex items-center gap-4">
                    <Button
                        onClick={handleSyncInstallations}
                        disabled={isSyncing || userData?.syncing}
                        variant="secondary"
                    >
                        <RiRefreshLine
                            className={`mr-2 size-4 ${isSyncing || userData?.syncing ? 'animate-spin' : ''}`}
                        />
                        {isSyncing || userData?.syncing
                            ? 'Syncing...'
                            : 'Sync Installations'}
                    </Button>
                    {userData?.synced_at ? (
                        <p className="text-xs text-gray-500">
                            Last synced:{' '}
                            {new Date(
                                userData.synced_at * 1000,
                            ).toLocaleString()}
                        </p>
                    ) : null}
                </div>
            </Card>

            {/* Logout Section */}
            <Card className="p-6 border-red-200 bg-red-50">
                <h2 className="text-xl font-semibold text-gray-900 mb-4">
                    Session
                </h2>
                <p className="text-gray-600 text-sm mb-4">
                    Sign out from your account. You'll need to authenticate
                    again to access this app.
                </p>
                <Button onClick={handleLogout} variant="destructive">
                    Sign Out
                </Button>
            </Card>
        </div>
    );
}
