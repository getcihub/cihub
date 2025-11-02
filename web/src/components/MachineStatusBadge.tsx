import { RiCheckboxCircleLine, RiCloseCircleLine, RiAlertLine, RiPauseLine } from '@remixicon/react'
import type { MachineStatus } from '../types/machine'
import { MachineStatusOnline, MachineStatusOffline, MachineStatusUnhealthy, MachineStatusPaused } from '../types/machine'
import { cx } from '../lib/utils'

interface MachineStatusBadgeProps {
    status: MachineStatus
    size?: 'sm' | 'md'
}

export function MachineStatusBadge({ status, size = 'md' }: MachineStatusBadgeProps) {
    const iconSize = size === 'sm' ? 'size-3' : 'size-4'

    switch (status) {
        case MachineStatusOnline:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-50 border border-green-200')}>
                    <RiCheckboxCircleLine className={cx(iconSize, 'text-green-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-green-700">Online</span>
                </div>
            )
        case MachineStatusOffline:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-gray-50 border border-gray-200')}>
                    <RiCloseCircleLine className={cx(iconSize, 'text-gray-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-gray-700">Offline</span>
                </div>
            )
        case MachineStatusUnhealthy:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-red-50 border border-red-200')}>
                    <RiAlertLine className={cx(iconSize, 'text-red-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-red-700">Unhealthy</span>
                </div>
            )
        case MachineStatusPaused:
            return (
                <div className={cx('inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-yellow-50 border border-yellow-200')}>
                    <RiPauseLine className={cx(iconSize, 'text-yellow-600')} aria-hidden="true" />
                    <span className="text-sm font-medium text-yellow-700">Paused</span>
                </div>
            )
    }
}
