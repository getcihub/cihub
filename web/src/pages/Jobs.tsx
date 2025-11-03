import { useState, useEffect } from 'react'
import { useNavigate, useSearch } from '@tanstack/react-router'
import { RiBriefcaseLine, RiArrowRightLine } from '@remixicon/react'
import { useJobs } from '../hooks/useJobs'
import { useInstallation } from '../hooks/useInstallation'
import { JobStatusBadge } from '../components/JobStatusBadge'
import { Card } from '../components/Card'
import { Skeleton } from '../components/Skeleton'
import { JobStatusQueued, JobStatusWaiting, JobStatusInProgress, JobStatusCompleted } from '../types/job'

const JOBS_PAGE_SIZE = 25

export function JobsPage() {
    const navigate = useNavigate()
    const { selectedInstallation } = useInstallation()
    const searchParams = useSearch({ from: '/$login/jobs' }) as { tab?: string } | undefined
    const initialTab = (searchParams?.tab as 'incomplete' | 'completed') || 'incomplete'

    const [activeTab, setActiveTab] = useState<'incomplete' | 'completed'>(initialTab)
    const [completedCursor, setCompletedCursor] = useState<number>(0)

    // Update URL when tab changes
    useEffect(() => {
        navigate({
            to: '/$login/jobs',
            params: { login: selectedInstallation!.login },
            search: { tab: activeTab },
            replace: true,
        })
    }, [activeTab, navigate, selectedInstallation])

    // Fetch incomplete jobs
    const {
        data: incompleteData = { data: [], hasMore: false },
        isLoading: incompleteLoading,
        error: incompleteError,
    } = useJobs({ status: 'incomplete' })

    // Fetch completed jobs with pagination
    const {
        data: completedData = { data: [], hasMore: false },
        isLoading: completedLoading,
        error: completedError,
    } = useJobs({ status: 'completed', limit: JOBS_PAGE_SIZE, jobId: completedCursor })

    const incompleteJobs = incompleteData.data || []
    const completedJobs = completedData.data || []

    // Filter incomplete jobs by status for statistics
    const queuedJobs = incompleteJobs.filter((j) => j.status === JobStatusQueued)
    const waitingJobs = incompleteJobs.filter((j) => j.status === JobStatusWaiting)
    const inProgressJobs = incompleteJobs.filter((j) => j.status === JobStatusInProgress)

    // Sort incomplete jobs by status priority: in_progress first, then queued, then waiting
    const statusPriority: Record<string, number> = {
        [JobStatusInProgress]: 0,
        [JobStatusQueued]: 1,
        [JobStatusWaiting]: 2,
        [JobStatusCompleted]: 3,
    }

    const sortedIncompleteJobs = [...incompleteJobs].sort((a, b) => {
        const priorityA = statusPriority[a.status] ?? 999
        const priorityB = statusPriority[b.status] ?? 999
        return priorityA - priorityB
    })

    // Calculate statistics
    const totalJobs = incompleteJobs.length
    const activeJobs = inProgressJobs.length + waitingJobs.length

    const handleJobClick = (jobId: string) => {
        navigate({ to: '/$login/jobs/$jobId', params: { login: selectedInstallation!.login, jobId } })
    }

    const handleLoadMoreCompleted = () => {
        if (completedJobs.length > 0) {
            const lastJobId = completedJobs[completedJobs.length - 1].id
            setCompletedCursor(lastJobId)
        }
    }

    const isLoading = activeTab === 'incomplete' ? incompleteLoading : completedLoading
    const error = activeTab === 'incomplete' ? incompleteError : completedError
    const hasMore = activeTab === 'incomplete' ? false : completedData.hasMore

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
                {(['incomplete', 'completed'] as const).map((status) => (
                    <button
                        key={status}
                        onClick={() => {
                            setActiveTab(status)
                            if (status === 'completed') {
                                setCompletedCursor(0)
                            }
                        }}
                        className={`px-4 py-2 rounded-lg font-medium text-sm whitespace-nowrap transition-all ${
                            activeTab === status
                                ? 'bg-black text-white'
                                : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                        }`}
                    >
                        <span className="capitalize">{status === 'incomplete' ? 'Active' : 'Completed'} Jobs</span>
                    </button>
                ))}
            </div>

            {/* Incomplete Jobs Section */}
            {activeTab === 'incomplete' && (
                <>
                    {/* Statistics Cards */}
                    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                        <Card className="p-6">
                            <div className="flex items-start justify-between">
                                <div>
                                    <p className="text-sm font-medium text-gray-600">Total Active Jobs</p>
                                    <div className="mt-2">
                                        <p className="text-3xl font-bold text-gray-900">{totalJobs}</p>
                                        <p className="text-sm text-gray-500 mt-1">
                                            {activeJobs} running or waiting
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
                                <div className="rounded-lg bg-green-100 p-3">
                                    <RiBriefcaseLine className="size-6 text-green-600" aria-hidden="true" />
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
                                <div className="rounded-lg bg-yellow-100 p-3">
                                    <RiBriefcaseLine className="size-6 text-yellow-600" aria-hidden="true" />
                                </div>
                            </div>
                        </Card>
                    </div>

                    {/* Incomplete Jobs List */}
                    <div>
                        {incompleteJobs.length === 0 ? (
                            <Card className="p-8 text-center">
                                <RiBriefcaseLine className="size-12 text-gray-300 mx-auto mb-4" aria-hidden="true" />
                                <p className="text-lg font-medium text-gray-900 mb-2">No active jobs</p>
                                <p className="text-gray-600">All jobs have been completed. Trigger a workflow to get started.</p>
                            </Card>
                        ) : (
                            <div className="space-y-3">
                                {sortedIncompleteJobs.map((job) => (
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
                </>
            )}

            {/* Completed Jobs Section */}
            {activeTab === 'completed' && (
                <div>
                    {completedJobs.length === 0 ? (
                        <Card className="p-8 text-center">
                            <RiBriefcaseLine className="size-12 text-gray-300 mx-auto mb-4" aria-hidden="true" />
                            <p className="text-lg font-medium text-gray-900 mb-2">No completed jobs</p>
                            <p className="text-gray-600">You haven't completed any jobs yet.</p>
                        </Card>
                    ) : (
                        <div className="space-y-3">
                            {completedJobs.map((job) => (
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

                            {/* Load More Button */}
                            {hasMore && (
                                <button
                                    onClick={handleLoadMoreCompleted}
                                    disabled={completedLoading}
                                    className="w-full px-4 py-3 text-center font-medium text-gray-700 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                                >
                                    {completedLoading ? 'Loading...' : 'Load More'}
                                    {!completedLoading && <RiArrowRightLine className="size-4" />}
                                </button>
                            )}
                        </div>
                    )}
                </div>
            )}
        </div>
    )
}
