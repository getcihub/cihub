import { useEffect } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { useInstallations } from '../hooks/useInstallations'
import { useInstallation } from '../hooks/useInstallation'
import { Card } from '../components/Card'
import { Button } from '../components/Button'
import { Skeleton } from '../components/Skeleton'
import { RiAddLine } from '@remixicon/react'

export function InstallationsPage() {
    const navigate = useNavigate()
    const { data: installations = [], isLoading, error } = useInstallations()
    const { selectInstallation, selectedInstallation, isLoading: installationLoading } = useInstallation()

    // Auto-redirect to selected installation if available
    useEffect(() => {
        if (!installationLoading && selectedInstallation) {
            navigate({ to: '/$login/machines', params: { login: selectedInstallation.login } })
        }
    }, [selectedInstallation, installationLoading, navigate])

    const handleSelectInstallation = async (installationId: number) => {
        const installation = installations.find((i) => i.id === installationId)
        if (!installation) return

        try {
            await selectInstallation(installation)
            navigate({ to: '/$login/machines', params: { login: installation.login } })
        } catch (err) {
            console.error('Failed to select installation:', err)
        }
    }

    const handleAddInstallation = () => {
        // Redirect to GitHub App installation flow
        // This would typically open the GitHub installation page
        window.location.href = '/auth/install'
    }

    if (isLoading) {
        return (
            <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                <h1 className="text-3xl font-bold text-gray-900 mb-8">Select an Installation</h1>
                <div className="space-y-4">
                    {[...Array(3)].map((_, i) => (
                        <Skeleton key={i} className="h-24 w-full rounded-lg" />
                    ))}
                </div>
            </div>
        )
    }

    if (error) {
        return (
            <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                <h1 className="text-3xl font-bold text-gray-900 mb-8">Select an Installation</h1>
                <Card className="bg-red-50 border-red-200 p-6">
                    <p className="text-red-800">
                        Failed to load installations. Please try again later.
                    </p>
                </Card>
            </div>
        )
    }

    if (installations.length === 0) {
        return (
            <div className="mx-auto max-w-2xl">
                <div className="mb-8">
                    <h1 className="text-3xl font-bold text-gray-900">Select an Installation</h1>
                </div>
                <Card className="p-8">
                    <div className="text-center">
                        <div className="mx-auto w-fit mb-4 rounded-lg bg-gray-100 p-3">
                            <svg
                                className="size-6 text-gray-400"
                                fill="none"
                                viewBox="0 0 24 24"
                                stroke="currentColor"
                            >
                                <path
                                    strokeLinecap="round"
                                    strokeLinejoin="round"
                                    strokeWidth={2}
                                    d="M12 6v6m0 0v6m0-6h6m0 0h6M6 12h6m0 0H6"
                                />
                            </svg>
                        </div>
                        <h3 className="text-lg font-semibold text-gray-900 mb-2">
                            No Installations Yet
                        </h3>
                        <p className="text-gray-600 mb-2">
                            You don't have access to any installations yet.
                        </p>
                        <p className="text-sm text-gray-500 mb-6">
                            Add a new installation to get started, or contact your organization administrator to grant you access.
                        </p>
                        <Button onClick={handleAddInstallation}>
                            <RiAddLine className="mr-2 size-4" />
                            Add Installation
                        </Button>
                    </div>
                </Card>
            </div>
        )
    }

    return (
        <div className="mx-auto max-w-4xl">
            <div className="mb-8 flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900">Select an Installation</h1>
                    <p className="text-gray-600 mt-2">Choose an installation to continue</p>
                </div>
                <Button onClick={handleAddInstallation} variant="secondary">
                    <RiAddLine className="mr-2 size-4" />
                    Add Installation
                </Button>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {installations.map((installation) => (
                    <button
                        key={installation.id}
                        onClick={() => handleSelectInstallation(installation.id)}
                        className="text-left"
                    >
                        <Card className="p-6 hover:shadow-lg hover:border-gray-300 transition-all cursor-pointer h-full">
                            <div className="flex items-start gap-4">
                                {installation.avatar_url ? (
                                    <img
                                        src={installation.avatar_url}
                                        alt={installation.login}
                                        className="size-16 rounded-lg border border-gray-200 object-cover flex-shrink-0"
                                    />
                                ) : (
                                    <div className="size-16 rounded-lg bg-gradient-to-br from-blue-400 to-blue-600 flex items-center justify-center flex-shrink-0 text-white font-semibold text-lg">
                                        {installation.login.charAt(0).toUpperCase()}
                                    </div>
                                )}
                                <div className="flex-1 min-w-0">
                                    <h2 className="text-lg font-semibold text-gray-900 truncate">
                                        {installation.login}
                                    </h2>
                                    <p className="text-sm text-gray-500 truncate mt-1">
                                        {installation.account_type}
                                    </p>
                                    <p className="text-xs text-gray-400 mt-3">
                                        Click to select
                                    </p>
                                </div>
                            </div>
                        </Card>
                    </button>
                ))}
            </div>
        </div>
    )
}
