import { RiCheckboxCircleLine, RiTimeLine, RiPlayCircleLine } from '@remixicon/react'
import type { JobStatus } from '../types/job'
import { JobStatusQueued, JobStatusWaiting, JobStatusInProgress, JobStatusCompleted } from '../types/job'
import { cx } from '../lib/utils'

interface JobStatusBadgeProps {
    status: JobStatus
    size?: 'sm' | 'md'
}

export function JobStatusBadge({ status, size = 'md' }: JobStatusBadgeProps) {
    const iconSize = size === 'sm' ? 'size-3' : 'size-4'

    switch (status) {
        case JobStatusInProgress:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-blue-50 border border-blue-200')}>
                    <RiPlayCircleLine className={cx(iconSize, 'text-blue-600 animate-pulse')} aria-hidden="true" />
                    <span className="text-sm font-medium text-blue-700">In Progress</span>
                </div>
            )
        case JobStatusWaiting:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-yellow-50 border border-yellow-200')}>
                    <RiTimeLine className={cx(iconSize, 'text-yellow-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-yellow-700">Waiting</span>
                </div>
            )
        case JobStatusQueued:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-gray-50 border border-gray-200')}>
                    <RiTimeLine className={cx(iconSize, 'text-gray-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-gray-700">Queued</span>
                </div>
            )
        case JobStatusCompleted:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-50 border border-green-200')}>
                    <RiCheckboxCircleLine className={cx(iconSize, 'text-green-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-green-700">Completed</span>
                </div>
            )
    }
}
