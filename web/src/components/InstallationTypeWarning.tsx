import { RiAlertLine } from '@remixicon/react'
import { useInstallation } from '../hooks/useInstallation'
import { InstallationTypeUser } from '../types/installation'

export function InstallationTypeWarning() {
    const { selectedInstallation } = useInstallation()

    if (!selectedInstallation || selectedInstallation.account_type !== InstallationTypeUser) {
        return null
    }

    return (
        <div className="bg-yellow-50 border-b border-yellow-200 px-4 sm:px-6 py-3">
            <div className="mx-auto max-w-7xl flex items-start gap-3">
                <RiAlertLine className="size-5 text-yellow-600 flex-shrink-0 mt-0.5" aria-hidden="true" />
                <div className="flex-1">
                    <p className="text-sm font-medium text-yellow-800">
                        CIHub only supports organization installations
                    </p>
                    <p className="text-sm text-yellow-700 mt-1">
                        User installations are not currently supported. Please install CIHub on an organization to use all features.
                    </p>
                </div>
            </div>
        </div>
    )
}
