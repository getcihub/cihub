import { useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { RiServerLine, RiCpuLine, RiRam2Line, RiAddLine, RiArrowRightSLine } from '@remixicon/react'
import { useMachines } from '../hooks/useMachines'
import { useInstallation } from '../hooks/useInstallation'
import { Card } from '../components/Card'
import { Skeleton } from '../components/Skeleton'
import { Button } from '../components/Button'
import { MachineStatusOnline } from '../types/machine'
import { MembershipRoleAdmin } from '../types/installation'

export function MachinesPage() {
    const navigate = useNavigate()
    const { selectedInstallation } = useInstallation()
    const { data: machines = [], isLoading, error } = useMachines()
    const [selectedStatus, setSelectedStatus] = useState<string>('all')

    const isAdmin = selectedInstallation?.membership?.role === MembershipRoleAdmin

    const handleAddMachine = () => {
        if (selectedInstallation) {
            navigate({ to: '/$login/machines/add', params: { login: selectedInstallation.login } })
        }
    }

    // Helper function to get runner count from machine
    const getMachineRunnerCount = (machine: { runners?: any[] }) => {
        const machineRunners = machine.runners ?? []
        return { runnerCount: machineRunners.length }
    }

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

    // Calculate statistics
    const totalMachines = machines.length
    const onlineMachines = machines.filter((m) => m.status === MachineStatusOnline).length
    const totalCPU = machines.reduce((sum, m) => sum + m.cpu, 0)
    const totalCPUAllocated = machines.reduce((sum, m) => sum + m.cpu_allocated, 0)
    const totalRAMAvailable = machines.reduce((sum, m) => sum + m.ram_available, 0)
    const totalRAMAllocated = machines.reduce((sum, m) => sum + m.ram_allocated, 0)

    const cpuUsagePercent = totalCPU > 0 ? Math.round((totalCPUAllocated / totalCPU) * 100) : 0
    const ramUsagePercent = totalRAMAvailable > 0 ? Math.round((totalRAMAllocated / totalRAMAvailable) * 100) : 0
    const ramUsageGB = Math.round(totalRAMAllocated / 1024)
    const totalRAMGB = Math.round(totalRAMAvailable / 1024)

    const handleMachineClick = (machineName: string) => {
        navigate({ to: '/$login/machines/$name', params: { login: selectedInstallation!.login, name: machineName } })
    }

    // Helper function to count machines by status
    const getStatusCount = (status: string) => {
        if (status === 'all') {
            return machines.length
        }
        return machines.filter((m) => m.status === status).length
    }

    // Filter machines based on selected status
    const filteredMachines = selectedStatus === 'all'
        ? machines
        : machines.filter((m) => m.status === selectedStatus)

    if (isLoading) {
        return (
            <div className="space-y-8">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900">Machines</h1>
                    <p className="text-gray-600 mt-2">Manage your self-hosted runners</p>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    {[...Array(3)].map((_, i) => (
                        <Skeleton key={i} className="h-32 w-full rounded-lg" />
                    ))}
                </div>
                <div className="space-y-4">
                    {[...Array(5)].map((_, i) => (
                        <Skeleton key={i} className="h-16 w-full rounded-lg" />
                    ))}
                </div>
            </div>
        )
    }

    if (error) {
        return (
            <div className="space-y-4">
                <h1 className="text-3xl font-bold text-gray-900">Machines</h1>
                <Card className="bg-red-50 border-red-200 p-6">
                    <p className="text-red-800">
                        Failed to load machines. Please try again later.
                    </p>
                </Card>
            </div>
        )
    }

    return (
        <div className="space-y-8">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900">Machines</h1>
                    <p className="text-gray-600 mt-2">Manage your self-hosted runners</p>
                </div>
                {isAdmin && (
                    <Button onClick={handleAddMachine} className="gap-2">
                        <RiAddLine className="size-4" />
                        Add Machine
                    </Button>
                )}
            </div>

            {/* Filter Bar */}
            <div className="flex gap-2 pb-2 overflow-x-auto">
                {['all', 'online', 'offline', 'unhealthy', 'paused'].map((status) => (
                    <button
                        key={status}
                        onClick={() => setSelectedStatus(status)}
                        className={`px-4 py-2 rounded-lg font-medium text-sm whitespace-nowrap transition-all ${
                            selectedStatus === status
                                ? 'bg-black text-white border-b-2 border-black'
                                : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                        }`}
                    >
                        <span className="capitalize">{status === 'all' ? 'All Machines' : status}</span>
                        <span className="ml-2 text-xs opacity-75">({getStatusCount(status)})</span>
                    </button>
                ))}
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <Card className="p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="text-sm font-medium text-gray-600">Total Machines</p>
                            <div className="mt-2">
                                <p className="text-3xl font-bold text-gray-900">{totalMachines}</p>
                                <p className="text-sm text-gray-500 mt-1">
                                    {onlineMachines} online
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-blue-100 p-3">
                            <RiServerLine className="size-6 text-blue-600" aria-hidden="true" />
                        </div>
                    </div>
                </Card>

                <Card className="p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="text-sm font-medium text-gray-600">CPU Usage</p>
                            <div className="mt-2">
                                <p className="text-3xl font-bold text-gray-900">{cpuUsagePercent}%</p>
                                <p className="text-sm text-gray-500 mt-1">
                                    {totalCPUAllocated} / {totalCPU} vCPU
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-purple-100 p-3">
                            <RiCpuLine className="size-6 text-purple-600" aria-hidden="true" />
                        </div>
                    </div>
                    <div className="mt-4 h-2 bg-gray-200 rounded-full overflow-hidden">
                        <div
                            className="h-full bg-purple-600 transition-all"
                            style={{ width: `${cpuUsagePercent}%` }}
                        />
                    </div>
                </Card>

                <Card className="p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="text-sm font-medium text-gray-600">RAM Usage</p>
                            <div className="mt-2">
                                <p className="text-3xl font-bold text-gray-900">{ramUsagePercent}%</p>
                                <p className="text-sm text-gray-500 mt-1">
                                    {ramUsageGB} / {totalRAMGB} GB
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-orange-100 p-3">
                            <RiRam2Line className="size-6 text-orange-600" aria-hidden="true" />
                        </div>
                    </div>
                    <div className="mt-4 h-2 bg-gray-200 rounded-full overflow-hidden">
                        <div
                            className="h-full bg-orange-600 transition-all"
                            style={{ width: `${ramUsagePercent}%` }}
                        />
                    </div>
                </Card>
            </div>

            {filteredMachines.length === 0 ? (
                <Card className="p-8 text-center">
                    <RiServerLine className="size-12 text-gray-300 mx-auto mb-4" aria-hidden="true" />
                    <p className="text-lg font-medium text-gray-900 mb-2">
                        {machines.length === 0 ? 'No machines yet' : `No ${selectedStatus} machines`}
                    </p>
                    <p className="text-gray-600 mb-6">
                        {machines.length === 0
                            ? 'No machines have been registered. Install the CIHub agent to get started.'
                            : `No machines with status "${selectedStatus}" found.`}
                    </p>
                    {isAdmin && machines.length === 0 && (
                        <Button onClick={handleAddMachine} className="gap-2">
                            <RiAddLine className="size-4" />
                            Create Machine
                        </Button>
                    )}
                </Card>
            ) : (
                <div className="space-y-3">
                    {filteredMachines.map((machine) => {
                        const { runnerCount } = getMachineRunnerCount(machine)

                        // Determine effective limits (use total if limit is 0, which means "unknown")
                        const cpuLimit = machine.cpu_limit > 0 ? machine.cpu_limit : machine.cpu
                        const ramLimit = machine.ram_limit > 0 ? machine.ram_limit : machine.ram_available

                        const cpuPercent = cpuLimit > 0 ? Math.round((machine.cpu_allocated / cpuLimit) * 100) : 0
                        const ramPercent = ramLimit > 0 ? Math.round((machine.ram_allocated / ramLimit) * 100) : 0

                        return (
                            <Card
                                key={machine.name}
                                onClick={() => handleMachineClick(machine.name)}
                                className="p-4 hover:shadow-md hover:border-gray-300 cursor-pointer transition-all"
                            >
                                <div className="flex items-stretch gap-4 justify-between">
                                    {/* Left side: Machine info */}
                                    <div className="flex gap-3 flex-1 min-w-0">
                                        {/* Status dot */}
                                        <div className={`h-2 w-2 rounded-full flex-shrink-0 mt-1.5 ${getStatusDotColor(machine.status)}`} />

                                        {/* Machine name and details */}
                                        <div className="min-w-0 flex-1">
                                            {/* Machine name */}
                                            <h3 className="text-sm font-semibold text-gray-900 font-mono truncate">{machine.name}</h3>

                                            {/* Architecture, runners count, and labels */}
                                            <div className="flex items-center gap-1 flex-wrap mt-1">
                                                <span className="text-xs text-gray-500 px-1.5 py-0.5 bg-gray-100 rounded">{machine.arch}</span>
                                                <span className="text-xs text-gray-600">•</span>
                                                <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-blue-50 border border-blue-200 rounded text-xs font-medium text-blue-900">
                                                    {runnerCount} runner{runnerCount !== 1 ? 's' : ''}
                                                </span>
                                                {machine.labels && machine.labels.length > 0 && (
                                                    <>
                                                        <span className="text-xs text-gray-600">•</span>
                                                        {machine.labels.map((label) => (
                                                            <span
                                                                key={label}
                                                                className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-700"
                                                            >
                                                                {label}
                                                            </span>
                                                        ))}
                                                    </>
                                                )}
                                            </div>
                                        </div>
                                    </div>

                                    {/* Right side: Resources (smaller) */}
                                    <div className="flex items-center gap-4 flex-shrink-0">
                                        {/* CPU Usage */}
                                        <div className="min-w-[95px]">
                                            <div className="flex items-center justify-between mb-0.5">
                                                <div className="flex items-center gap-0.5">
                                                    <RiCpuLine className="size-2.5 text-blue-600" aria-hidden="true" />
                                                    <p className="text-xs text-gray-600">CPU</p>
                                                </div>
                                                <p className="text-xs font-medium text-gray-900">{cpuPercent}%</p>
                                            </div>
                                            <div className="h-1 bg-gray-200 rounded-full overflow-hidden">
                                                <div
                                                    className="h-full bg-purple-600 transition-all"
                                                    style={{ width: `${cpuPercent}%` }}
                                                />
                                            </div>
                                            <p className="text-xs text-gray-500 mt-0.5">
                                                {machine.cpu > 0 ? `${machine.cpu_allocated}/${cpuLimit}` : 'Unknown'}
                                            </p>
                                        </div>

                                        {/* RAM Usage */}
                                        <div className="min-w-[95px]">
                                            <div className="flex items-center justify-between mb-0.5">
                                                <div className="flex items-center gap-0.5">
                                                    <RiRam2Line className="size-2.5 text-blue-600" aria-hidden="true" />
                                                    <p className="text-xs text-gray-600">RAM</p>
                                                </div>
                                                <p className="text-xs font-medium text-gray-900">{ramPercent}%</p>
                                            </div>
                                            <div className="h-1 bg-gray-200 rounded-full overflow-hidden">
                                                <div
                                                    className="h-full bg-orange-600 transition-all"
                                                    style={{ width: `${ramPercent}%` }}
                                                />
                                            </div>
                                            <p className="text-xs text-gray-500 mt-0.5">
                                                {machine.ram_available > 0 ? `${Math.round(machine.ram_allocated / 1024)}GB/${Math.round(ramLimit / 1024)}GB` : 'Unknown'}
                                            </p>
                                        </div>

                                        {/* Arrow icon */}
                                        <RiArrowRightSLine className="size-4 text-gray-400 flex-shrink-0" aria-hidden="true" />
                                    </div>
                                </div>
                            </Card>
                        )
                    })}
                </div>
            )}
        </div>
    )
}
