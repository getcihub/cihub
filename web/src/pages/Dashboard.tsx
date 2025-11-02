import { useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { RiBriefcaseLine } from '@remixicon/react'
import { useJobs } from '../hooks/useJobs'
import { useInstallation } from '../hooks/useInstallation'
import { JobStatusBadge } from '../components/JobStatusBadge'
import { Card } from '../components/Card'
import { Skeleton } from '../components/Skeleton'
import { JobStatusQueued, JobStatusWaiting, JobStatusInProgress, JobStatusCompleted } from '../types/job'

export function DashboardPage() {
    const navigate = useNavigate()
    const { selectedInstallation } = useInstallation()
    const { data: jobs = [], isLoading, error } = useJobs()
    const [selectedStatus, setSelectedStatus] = useState<string>('all')

    // Filter jobs by status
    const queuedJobs = jobs.filter((j) => j.status === JobStatusQueued)
    const waitingJobs = jobs.filter((j) => j.status === JobStatusWaiting)
    const inProgressJobs = jobs.filter((j) => j.status === JobStatusInProgress)

    // Helper function to count jobs by status
    const getStatusCount = (status: string) => {
        if (status === 'all') {
            return jobs.length
        }
        return jobs.filter((j) => j.status === status).length
    }

    // Filter jobs based on selected status
    const filteredJobs = selectedStatus === 'all'
        ? jobs
        : jobs.filter((j) => j.status === selectedStatus)

    // Sort jobs by status priority: in_progress first, then queued, then waiting
    const statusPriority: Record<string, number> = {
        [JobStatusInProgress]: 0,
        [JobStatusQueued]: 1,
        [JobStatusWaiting]: 2,
        [JobStatusCompleted]: 3,
    }

    const sortedFilteredJobs = [...filteredJobs].sort((a, b) => {
        const priorityA = statusPriority[a.status] ?? 999
        const priorityB = statusPriority[b.status] ?? 999
        return priorityA - priorityB
    })

    // Calculate statistics
    const totalJobs = jobs.length
    const activeJobs = inProgressJobs.length + waitingJobs.length

    const handleJobClick = (jobId: string) => {
        navigate({ to: '/$login/jobs/$jobId', params: { login: selectedInstallation!.login, jobId } })
    }

    if (isLoading) {
        return (
            <div className="space-y-8">
                <div>
                    <h1 className="text-3xl font-bold text-gray-900">Jobs</h1>
                    <p className="text-gray-600 mt-2">Manage and monitor your job queue</p>
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
                <h1 className="text-3xl font-bold text-gray-900">Jobs</h1>
                <Card className="bg-red-50 border-red-200 p-6">
                    <p className="text-red-800">
                        Failed to load jobs. Please try again later.
                    </p>
                </Card>
            </div>
        )
    }

    return (
        <div className="space-y-8">
            <div>
                <h1 className="text-3xl font-bold text-gray-900">Jobs</h1>
                <p className="text-gray-600 mt-2">Manage and monitor your job queue</p>
            </div>

            {/* Filter Bar */}
            <div className="flex gap-2 pb-2 overflow-x-auto">
                {['all', JobStatusInProgress, JobStatusQueued, JobStatusWaiting, JobStatusCompleted].map((status) => (
                    <button
                        key={status}
                        onClick={() => setSelectedStatus(status)}
                        className={`px-4 py-2 rounded-lg font-medium text-sm whitespace-nowrap transition-all cursor-pointer ${
                            selectedStatus === status
                                ? 'bg-black text-white border-b-2 border-black'
                                : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                        }`}
                    >
                        <span className="capitalize">{status === 'all' ? 'All Jobs' : status.replace('_', ' ')}</span>
                        <span className="ml-2 text-xs opacity-75">({getStatusCount(status)})</span>
                    </button>
                ))}
            </div>

            {/* Statistics Cards */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <Card className="p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="text-sm font-medium text-gray-600">Total Jobs</p>
                            <div className="mt-2">
                                <p className="text-3xl font-bold text-gray-900">{totalJobs}</p>
                                <p className="text-sm text-gray-500 mt-1">
                                    {activeJobs} active
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-blue-100 p-3">
                            <RiBriefcaseLine className="size-6 text-blue-600" aria-hidden="true" />
                        </div>
                    </div>
                </Card>

                <Card className="p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="text-sm font-medium text-gray-600">In Progress</p>
                            <div className="mt-2">
                                <p className="text-3xl font-bold text-gray-900">{inProgressJobs.length}</p>
                                <p className="text-sm text-gray-500 mt-1">
                                    Currently running
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-blue-100 p-3">
                            <RiBriefcaseLine className="size-6 text-blue-600" aria-hidden="true" />
                        </div>
                    </div>
                </Card>

                <Card className="p-6">
                    <div className="flex items-start justify-between">
                        <div>
                            <p className="text-sm font-medium text-gray-600">Queued</p>
                            <div className="mt-2">
                                <p className="text-3xl font-bold text-gray-900">{queuedJobs.length}</p>
                                <p className="text-sm text-gray-500 mt-1">
                                    Waiting to run
                                </p>
                            </div>
                        </div>
                        <div className="rounded-lg bg-gray-100 p-3">
                            <RiBriefcaseLine className="size-6 text-gray-600" aria-hidden="true" />
                        </div>
                    </div>
                </Card>
            </div>

            {/* Jobs List */}
            <div>
                {filteredJobs.length === 0 ? (
                    <Card className="p-8 text-center">
                        <RiBriefcaseLine className="size-12 text-gray-300 mx-auto mb-4" aria-hidden="true" />
                        <p className="text-lg font-medium text-gray-900 mb-2">No jobs yet</p>
                        <p className="text-gray-600">No jobs have been queued. Trigger a workflow to get started.</p>
                    </Card>
                ) : (
                    <div className="space-y-3">
                        {sortedFilteredJobs.map((job) => (
                            <Card
                                key={job.id}
                                className="p-4 hover:shadow-md hover:border-gray-300 cursor-pointer transition-all"
                                onClick={() => handleJobClick(String(job.id))}
                            >
                                <div className="flex items-center justify-between gap-3">
                                    <div className="flex items-center gap-3 flex-1 min-w-0">
                                        {/* Author Avatar */}
                                        <img
                                            src={job.author_avatar}
                                            alt={job.author_login}
                                            className="size-8 rounded-full flex-shrink-0 object-cover"
                                            title={job.author_login}
                                        />
                                        {/* Job Info */}
                                        <div className="flex-1 min-w-0">
                                            <p className="text-sm font-semibold text-gray-900 truncate">{job.name}</p>
                                            <div className="flex items-center gap-2 mt-1">
                                                <p className="text-xs text-gray-500 truncate">
                                                    {job.owner}/{job.repo}
                                                </p>
                                                <span className="text-xs text-gray-400">•</span>
                                                <p className="text-xs text-gray-500 truncate">
                                                    {job.branch}
                                                </p>
                                                <span className="text-xs text-gray-400">•</span>
                                                <p className="text-xs text-gray-500 font-mono truncate">
                                                    {job.sha.substring(0, 7)}
                                                </p>
                                            </div>
                                            <p className="text-xs text-gray-400 mt-0.5 truncate">
                                                {job.workflow} / {job.name}
                                            </p>
                                        </div>
                                    </div>
                                    {/* Status Badge */}
                                    <div className="flex-shrink-0">
                                        <JobStatusBadge status={job.status} size="sm" />
                                    </div>
                                </div>
                            </Card>
                        ))}
                    </div>
                )}
            </div>
        </div>
    )
}
