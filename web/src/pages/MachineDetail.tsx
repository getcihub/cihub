import { useState, useRef, useEffect } from 'react'
import { useNavigate, useParams } from '@tanstack/react-router'
import { RiArrowLeftLine, RiCpuLine, RiRam2Line, RiServerLine, RiMoreLine, RiAlertLine } from '@remixicon/react'
import { useMachineDetail } from '@/hooks/useMachineDetail'
import { useInstallation } from '@/hooks/useInstallation'
import { useMachineMutations } from '@/hooks/useMachineMutations'
import { Card } from '@/components/Card'
import { Button } from '@/components/Button'
import { Skeleton } from '@/components/Skeleton'
import { MembershipRoleAdmin } from '@/types/installation'

export function MachineDetailPage() {
    const navigate = useNavigate()
    const { name: machineName, login } = useParams({ from: '/$login/machines/$name' })
    const { selectedInstallation } = useInstallation()
    const { data: machine, isLoading, error } = useMachineDetail(machineName)
    const { pauseMachine, resumeMachine, restartMachine, deleteMachine } = useMachineMutations()
    const [showSettings, setShowSettings] = useState(false)
    const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)
    const settingsRef = useRef<HTMLDivElement>(null)

    // Close settings menu when clicking outside
    useEffect(() => {
        function handleClickOutside(event: MouseEvent) {
            if (settingsRef.current && !settingsRef.current.contains(event.target as Node)) {
                setShowSettings(false)
            }
        }

        if (showSettings) {
            document.addEventListener('mousedown', handleClickOutside)
            return () => {
                document.removeEventListener('mousedown', handleClickOutside)
            }
        }
    }, [showSettings])

    const isAdmin = selectedInstallation?.membership?.role === MembershipRoleAdmin

    // Use login from params or selectedInstallation as fallback
    const currentLogin = login || selectedInstallation?.login

    // Helper function to get status dot styling
    const getStatusDotColor = (status: string) => {
        switch (status) {
            case 'online':
                return 'bg-green-500 shadow-lg shadow-green-500/50 animate-pulse'
            case 'offline':
                return 'bg-gray-400'
            case 'unhealthy':
                return 'bg-red-500'
            case 'paused':
                return 'bg-yellow-500'
            default:
                return 'bg-gray-400'
        }
    }

    // Handler functions for settings
    const handlePauseMachine = async () => {
        try {
            await pauseMachine.mutateAsync(machineName)
            setShowSettings(false)
        } catch (error) {
            console.error('Failed to pause machine:', error)
        }
    }

    const handleResumeMachine = async () => {
        try {
            await resumeMachine.mutateAsync(machineName)
            setShowSettings(false)
        } catch (error) {
            console.error('Failed to resume machine:', error)
        }
    }

    const handleRestartMachine = async () => {
        try {
            await restartMachine.mutateAsync(machineName)
            setShowSettings(false)
        } catch (error) {
            console.error('Failed to restart machine:', error)
        }
    }

    const handleDeleteMachineClick = () => {
        setShowDeleteConfirm(true)
        setShowSettings(false)
    }

    const handleConfirmDelete = async () => {
        try {
            await deleteMachine.mutateAsync(machineName)
            setShowDeleteConfirm(false)
            // Navigate back to machines list after successful deletion
            navigate({ to: '/$login/machines', params: { login: currentLogin || 'org' } })
        } catch (error) {
            console.error('Failed to delete machine:', error)
        }
    }

    // Calculate resource usage from machine
    const machineRunners = machine?.runners ?? []

    // Determine effective limits (use total if limit is 0, which means "unknown")
    const cpuLimit = machine ? (machine.cpu_limit > 0 ? machine.cpu_limit : machine.cpu) : 0
    const ramLimit = machine ? (machine.ram_limit > 0 ? machine.ram_limit : machine.ram_available) : 0

    const cpuAllocated = machine?.cpu_allocated || 0
    const ramAllocated = machine?.ram_allocated || 0
    const cpuUsagePercent = cpuLimit > 0 ? Math.round((cpuAllocated / cpuLimit) * 100) : 0
    const ramUsagePercent = ramLimit > 0 ? Math.round((ramAllocated / ramLimit) * 100) : 0

    if (isLoading) {
        return (
            <div className="space-y-8">
                <button
                    onClick={() => navigate({ to: '/$login/machines', params: { login: currentLogin || 'org' } })}
                    className="text-blue-600 hover:text-blue-700 flex items-center gap-2 font-medium"
                >
                    <RiArrowLeftLine className="size-4" aria-hidden="true" />
                    Back to Machines
                </button>
                <Skeleton className="h-12 w-48 rounded-lg" />
                <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                    <div className="lg:col-span-2">
                        <Skeleton className="h-64 w-full rounded-lg" />
                    </div>
                    <div>
                        <Skeleton className="h-64 w-full rounded-lg" />
                    </div>
                </div>
            </div>
        )
    }

    if (error || !machine) {
        return (
            <div className="space-y-4">
                <button
                    onClick={() => navigate({ to: '/$login/machines', params: { login: currentLogin || 'org' } })}
                    className="text-blue-600 hover:text-blue-700 flex items-center gap-2 font-medium"
                >
                    <RiArrowLeftLine className="size-4" aria-hidden="true" />
                    Back to Machines
                </button>
                <Card className="bg-red-50 border-red-200 p-6">
                    <p className="text-red-800">
                        {error ? 'Failed to load machine details. Please try again later.' : 'Machine not found.'}
                    </p>
                </Card>
            </div>
        )
    }

    return (
        <div className="space-y-8">
            {/* Back Button */}
            <button
                onClick={() => navigate({ to: '/$login/machines', params: { login: currentLogin || 'org' } })}
                className="text-blue-600 hover:text-blue-700 flex items-center gap-2 font-medium"
            >
                <RiArrowLeftLine className="size-4" aria-hidden="true" />
                Back to Machines
            </button>

            {/* Header */}
            <div className="flex items-start justify-between gap-4">
                <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-3 min-w-0">
                        <h1 className="text-3xl font-bold text-gray-900 truncate">{machine.name}</h1>
                        {/* Status dot */}
                        <div className={`h-3 w-3 rounded-full flex-shrink-0 ${getStatusDotColor(machine.status)}`} />
                    </div>
                    <p className="text-sm text-gray-500 mt-1">
                        Last seen: {new Date(machine.last_seen_at * 1000).toLocaleString('en-US', {
                            year: 'numeric',
                            month: 'short',
                            day: 'numeric',
                            hour: '2-digit',
                            minute: '2-digit',
                        })}
                    </p>
                </div>
                {isAdmin && (
                    <div className="relative flex-shrink-0" ref={settingsRef}>
                        <button
                            onClick={() => setShowSettings(!showSettings)}
                            className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
                            title="Machine settings"
                        >
                            <RiMoreLine className="size-5 text-gray-600" aria-hidden="true" />
                        </button>
                        {showSettings && (
                            <div className="absolute right-0 mt-2 w-48 bg-white border border-gray-200 rounded-lg shadow-lg z-10">
                                <button
                                    onClick={handlePauseMachine}
                                    className="w-full text-left px-4 py-2 text-sm text-gray-900 hover:bg-gray-50 border-b border-gray-200 transition-colors"
                                >
                                    Pause Machine
                                </button>
                                <button
                                    onClick={handleResumeMachine}
                                    className="w-full text-left px-4 py-2 text-sm text-gray-900 hover:bg-gray-50 border-b border-gray-200 transition-colors"
                                >
                                    Resume Machine
                                </button>
                                <button
                                    onClick={handleRestartMachine}
                                    className="w-full text-left px-4 py-2 text-sm text-gray-900 hover:bg-gray-50 border-b border-gray-200 transition-colors"
                                >
                                    Restart Machine
                                </button>
                                <button
                                    onClick={handleDeleteMachineClick}
                                    className="w-full text-left px-4 py-2 text-sm text-red-600 hover:bg-red-50 transition-colors"
                                >
                                    Delete Machine
                                </button>
                            </div>
                        )}
                    </div>
                )}
            </div>

            {/* Main Grid */}
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* Left Column - Primary Info */}
                <div className="lg:col-span-2 space-y-6">
                    {/* Machine Details - Combined Card */}
                    <Card className="p-6">
                        {/* Labels */}
                        {machine.labels && machine.labels.length > 0 && (
                            <div className="mb-6 pb-6 border-b border-gray-200">
                                <p className="text-sm font-medium text-gray-600 mb-3">Labels</p>
                                <div className="flex gap-2 flex-wrap">
                                    {machine.labels.map((label) => (
                                        <span
                                            key={label}
                                            className="inline-flex items-center px-3 py-1 rounded-full text-xs font-medium bg-gray-100 text-gray-700"
                                        >
                                            {label}
                                        </span>
                                    ))}
                                </div>
                            </div>
                        )}

                        {/* Resources */}
                        <div>
                            <h2 className="text-sm font-semibold text-gray-900 mb-4">Resources</h2>
                            <div className="space-y-6">
                                {/* CPU */}
                                <div>
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2">
                                            <RiCpuLine className="size-5 text-blue-600" aria-hidden="true" />
                                            <span className="text-sm font-medium text-gray-600">CPU</span>
                                        </div>
                                        <div className="text-right">
                                            <p className="text-lg font-semibold text-gray-900">
                                                {machine.cpu > 0 ? `${cpuUsagePercent}%` : 'Unknown'}
                                            </p>
                                            <p className="text-xs text-gray-500">
                                                {machine.cpu > 0
                                                    ? `${cpuAllocated} / ${cpuLimit} vCPU`
                                                    : 'Data unavailable'}
                                            </p>
                                        </div>
                                    </div>
                                    {machine.cpu > 0 && (
                                        <div className="h-2 bg-gray-200 rounded-full overflow-hidden">
                                            <div
                                                className="h-full bg-blue-600 transition-all"
                                                style={{ width: `${cpuUsagePercent}%` }}
                                            />
                                        </div>
                                    )}
                                    {machine.cpu > 0 && (
                                        <div className="mt-3 text-xs text-gray-600 space-y-1">
                                            <div className="flex justify-between">
                                                <span>Allocated:</span>
                                                <span className="font-medium">{cpuAllocated} vCPU</span>
                                            </div>
                                            <div className="flex justify-between">
                                                <span>Available:</span>
                                                <span className="font-medium">{cpuLimit - cpuAllocated} vCPU</span>
                                            </div>
                                            {machine.cpu_limit > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Limit:</span>
                                                    <span className="font-medium">{cpuLimit} vCPU</span>
                                                </div>
                                            )}
                                            {machine.cpu > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Total:</span>
                                                    <span className="font-medium">{machine.cpu} vCPU</span>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>

                                {/* RAM */}
                                <div>
                                    <div className="flex items-center justify-between mb-3">
                                        <div className="flex items-center gap-2">
                                            <RiRam2Line className="size-5 text-blue-600" aria-hidden="true" />
                                            <span className="text-sm font-medium text-gray-600">RAM</span>
                                        </div>
                                        <div className="text-right">
                                            <p className="text-lg font-semibold text-gray-900">
                                                {machine.ram_available > 0 ? `${ramUsagePercent}%` : 'Unknown'}
                                            </p>
                                            <p className="text-xs text-gray-500">
                                                {machine.ram_available > 0
                                                    ? `${Math.round(ramAllocated / 1024)} / ${Math.round(ramLimit / 1024)} GB`
                                                    : 'Data unavailable'}
                                            </p>
                                        </div>
                                    </div>
                                    {machine.ram_available > 0 && (
                                        <div className="h-2 bg-gray-200 rounded-full overflow-hidden">
                                            <div
                                                className="h-full bg-blue-600 transition-all"
                                                style={{ width: `${ramUsagePercent}%` }}
                                            />
                                        </div>
                                    )}
                                    {machine.ram_available > 0 && (
                                        <div className="mt-3 text-xs text-gray-600 space-y-1">
                                            <div className="flex justify-between">
                                                <span>Allocated:</span>
                                                <span className="font-medium">{Math.round(ramAllocated / 1024)} GB</span>
                                            </div>
                                            <div className="flex justify-between">
                                                <span>Available:</span>
                                                <span className="font-medium">{Math.round((ramLimit - ramAllocated) / 1024)} GB</span>
                                            </div>
                                            {machine.ram_limit > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Limit:</span>
                                                    <span className="font-medium">{Math.round(ramLimit / 1024)} GB</span>
                                                </div>
                                            )}
                                            {machine.ram_available > 0 && (
                                                <div className="flex justify-between">
                                                    <span>Total:</span>
                                                    <span className="font-medium">{Math.round(machine.ram_available / 1024)} GB</span>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </div>
                            </div>
                        </div>
                    </Card>

                    {/* Runners on this Machine */}
                    <div>
                        <h2 className="text-lg font-semibold text-gray-900 mb-4">Runners on this Machine</h2>
                        {machineRunners.length === 0 ? (
                            <Card className="p-8 text-center">
                                <RiServerLine className="size-12 text-gray-300 mx-auto mb-4" aria-hidden="true" />
                                <p className="text-lg font-medium text-gray-900 mb-2">No runners yet</p>
                                <p className="text-gray-600">No runners have been assigned to this machine.</p>
                            </Card>
                        ) : (
                            <div className="space-y-3">
                                {machineRunners.map((runner) => (
                                    <Card key={runner.id} className="p-4 hover:bg-gray-50 transition-colors">
                                        <div className="flex items-center justify-between gap-4">
                                            <div className="flex-1 min-w-0">
                                                <p className="text-sm font-medium text-gray-900">{runner.name}</p>
                                                <div className="flex items-center gap-3 mt-2">
                                                    <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-50 text-blue-700 border border-blue-200 capitalize">
                                                        {runner.status}
                                                    </span>
                                                    <p className="text-xs text-gray-500">{runner.arch}</p>
                                                    <p className="text-xs text-gray-500">
                                                        {runner.cpu} vCPU • {Math.round(runner.ram / 1024)} GB RAM
                                                    </p>
                                                </div>
                                            </div>
                                        </div>
                                    </Card>
                                ))}
                            </div>
                        )}
                    </div>
                </div>

                {/* Right Column - Architecture & Dates */}
                <div className="space-y-6">
                    {/* Architecture Card */}
                    <Card className="p-6">
                        <h2 className="text-sm font-semibold text-gray-900 mb-4">Architecture</h2>
                        <div className="flex items-center gap-3">
                            <RiServerLine className="size-5 text-blue-600" aria-hidden="true" />
                            <span className="text-sm font-medium text-gray-700 capitalize">{machine.arch}</span>
                        </div>
                    </Card>

                    {/* Dates Card */}
                    <Card className="p-6">
                        <h2 className="text-sm font-semibold text-gray-900 mb-4">Dates</h2>
                        <div className="space-y-3">
                            <div>
                                <p className="text-xs text-gray-600 mb-1">Created</p>
                                <p className="text-sm text-gray-900">
                                    {new Date(machine.created_at * 1000).toLocaleString('en-US', {
                                        year: 'numeric',
                                        month: 'short',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    })}
                                </p>
                            </div>
                            <div>
                                <p className="text-xs text-gray-600 mb-1">Last Seen</p>
                                <p className="text-sm text-gray-900">
                                    {new Date(machine.last_seen_at * 1000).toLocaleString('en-US', {
                                        year: 'numeric',
                                        month: 'short',
                                        day: 'numeric',
                                        hour: '2-digit',
                                        minute: '2-digit',
                                    })}
                                </p>
                            </div>
                        </div>
                    </Card>
                </div>
            </div>

            {/* Delete Confirmation Modal */}
            {showDeleteConfirm && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                    <Card className="w-full max-w-md mx-4 p-6">
                        <div className="flex items-start gap-4 mb-6">
                            <div className="flex-shrink-0">
                                <RiAlertLine className="size-6 text-red-600" aria-hidden="true" />
                            </div>
                            <div className="flex-1">
                                <h3 className="text-lg font-semibold text-gray-900">Delete Machine</h3>
                                <p className="text-sm text-gray-600 mt-2">
                                    Are you sure you want to delete <span className="font-mono font-semibold">{machine?.name}</span>? This action cannot be undone.
                                </p>
                            </div>
                        </div>
                        <div className="flex gap-3">
                            <Button
                                onClick={() => setShowDeleteConfirm(false)}
                                variant="secondary"
                                disabled={deleteMachine.isPending}
                            >
                                Cancel
                            </Button>
                            <Button
                                onClick={handleConfirmDelete}
                                variant="primary"
                                disabled={deleteMachine.isPending}
                                className="bg-red-600 hover:bg-red-700 text-white"
                            >
                                {deleteMachine.isPending ? 'Deleting...' : 'Delete'}
                            </Button>
                        </div>
                    </Card>
                </div>
            )}
        </div>
    )
}
