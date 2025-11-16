import { Button } from '@/components/Button';
import { Card } from '@/components/Card';
import { Skeleton } from '@/components/Skeleton';
import { useInstallation } from '@/hooks/useInstallation';
import { useInstallations } from '@/hooks/useInstallations';
import { useUser } from '@/hooks/useUser';
import { useVarz } from '@/hooks/useVarz';
import { RiAddLargeLine, RiAddLine } from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
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

    if (isLoading) {
        return (
            <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                <h1 className="text-3xl font-bold text-gray-900 mb-8">
                    Select an Installation
                </h1>
                <div className="space-y-4">
                    {[...Array(3)].map((_, i) => (
                        <Skeleton key={i} className="h-24 w-full rounded-lg" />
                    ))}
                </div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                <h1 className="text-3xl font-bold text-gray-900 mb-8">
                    Select an Installation
                </h1>
                <Card className="bg-red-50 border-red-200 p-6">
                    <p className="text-red-800">
                        Failed to load installations. Please try again later.
                    </p>
                </Card>
            </div>
        );
    }

    if (installations.length === 0) {
        // Show loading state if user data is being synced
        if (user?.syncing) {
            return (
                <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                    <h1 className="text-3xl font-bold text-gray-900 mb-8">
                        Select an Installation
                    </h1>
                    <Card className="p-8">
                        <div className="text-center">
                            <div className="mx-auto w-fit mb-4">
                                <div className="inline-block">
                                    <svg
                                        className="size-8 text-blue-500 animate-spin"
                                        xmlns="http://www.w3.org/2000/svg"
                                        fill="none"
                                        viewBox="0 0 24 24"
                                    >
                                        <circle
                                            className="opacity-25"
                                            cx="12"
                                            cy="12"
                                            r="10"
                                            stroke="currentColor"
                                            strokeWidth="4"
                                        />
                                        <path
                                            className="opacity-75"
                                            fill="currentColor"
                                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                                        />
                                    </svg>
                                </div>
                            </div>
                            <h3 className="text-lg font-semibold text-gray-900 mb-2">
                                Syncing Your Data
                            </h3>
                            <p className="text-gray-600">
                                We're fetching your installation data. Please
                                wait a moment.
                            </p>
                        </div>
                    </Card>
                </div>
            );
        }

        // Show empty state only when not syncing and no installations
        return (
            <div className="mx-auto max-w-2xl">
                <div className="mb-8">
                    <h1 className="text-3xl font-bold text-gray-900">
                        Select an Installation
                    </h1>
                </div>
                <Card className="p-8">
                    <div className="text-center">
                        <div className="mx-auto w-fit mb-4 rounded-lg bg-gray-100 p-3">
                            <RiAddLargeLine className="size-4" />
                        </div>
                        <h3 className="text-lg font-semibold text-gray-900 mb-2">
                            No Installations Yet
                        </h3>
                        <p className="text-gray-600 mb-2">
                            You don't have access to any installations yet.
                        </p>
                        <p className="text-sm text-gray-500 mb-6">
                            Add a new installation to get started, or contact
                            your organization administrator to grant you access.
                        </p>
                        <div className="flex flex-col gap-3 sm:flex-row sm:justify-center">
                            <Button onClick={handleAddInstallation}>
                                <RiAddLine className="mr-2 size-4" />
                                Add Installation
                            </Button>
                            <Button
                                onClick={handleSyncInstallations}
                                variant="secondary"
                                disabled={user?.syncing || isSyncingRequest}
                            >
                                {user?.syncing || isSyncingRequest
                                    ? 'Syncing...'
                                    : "Can't see your installation? Synchronize"}
                            </Button>
                        </div>
                    </div>
                </Card>
            </div>
        );
    }

    return (
        <div className="fixed inset-0 flex items-center justify-center px-4 sm:px-6 overflow-hidden">
            <div className="w-full max-w-2xl">
                <div className="text-center mb-12">
                    <h1 className="text-3xl font-bold text-gray-900">
                        Select an Installation
                    </h1>
                </div>

                <div className="space-y-6">
                    {/* Installations Grid */}
                    <div className="space-y-3">
                        {installations.map((installation) => (
                            <button
                                key={installation.id}
                                onClick={() =>
                                    handleSelectInstallation(installation.id)
                                }
                                className="w-full text-left"
                            >
                                <Card className="p-4 hover:shadow-md hover:border-gray-300 transition-all cursor-pointer">
                                    <div className="flex items-center gap-4">
                                        {installation.avatar_url ? (
                                            <img
                                                src={installation.avatar_url}
                                                alt={installation.login}
                                                className="size-12 rounded-lg border border-gray-200 object-cover flex-shrink-0"
                                            />
                                        ) : (
                                            <div className="size-12 rounded-lg bg-gradient-to-br from-blue-400 to-blue-600 flex items-center justify-center flex-shrink-0 text-white font-semibold">
                                                {installation.login
                                                    .charAt(0)
                                                    .toUpperCase()}
                                            </div>
                                        )}
                                        <div className="flex-1 min-w-0">
                                            <h2 className="text-base font-semibold text-gray-900 truncate">
                                                {installation.login}
                                            </h2>
                                        </div>
                                        <div className="flex-shrink-0">
                                            <svg
                                                className="size-5 text-gray-400"
                                                fill="none"
                                                viewBox="0 0 24 24"
                                                stroke="currentColor"
                                            >
                                                <path
                                                    strokeLinecap="round"
                                                    strokeLinejoin="round"
                                                    strokeWidth={2}
                                                    d="M9 5l7 7-7 7"
                                                />
                                            </svg>
                                        </div>
                                    </div>
                                </Card>
                            </button>
                        ))}
                    </div>

                    {/* Sync Section */}
                    {user?.syncing ? (
                        <div className="text-center py-8">
                            <div className="inline-block mb-4">
                                <svg
                                    className="size-8 text-blue-500 animate-spin"
                                    xmlns="http://www.w3.org/2000/svg"
                                    fill="none"
                                    viewBox="0 0 24 24"
                                >
                                    <circle
                                        className="opacity-25"
                                        cx="12"
                                        cy="12"
                                        r="10"
                                        stroke="currentColor"
                                        strokeWidth="4"
                                    />
                                    <path
                                        className="opacity-75"
                                        fill="currentColor"
                                        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                                    />
                                </svg>
                            </div>
                            <p className="text-gray-600 font-medium">
                                Syncing your installations...
                            </p>
                        </div>
                    ) : (
                        <div className="flex flex-col gap-3 justify-center">
                            <Button
                                onClick={handleSyncInstallations}
                                variant="secondary"
                                disabled={user?.syncing || isSyncingRequest}
                            >
                                {user?.syncing || isSyncingRequest
                                    ? 'Syncing...'
                                    : "Can't see your installation? Synchronize"}
                            </Button>
                            <Button onClick={handleAddInstallation}>
                                <RiAddLine className="mr-2 size-4" />
                                Add Installation
                            </Button>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
