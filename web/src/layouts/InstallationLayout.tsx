import { Outlet } from '@tanstack/react-router'
import { ProtectedInstallationRoute } from '../components/ProtectedInstallationRoute'
import { Navigation } from '../components/ui/Navigation'
import { InstallationTypeWarning } from '../components/InstallationTypeWarning'

/**
 * InstallationLayout wraps routes that require an installation context
 * Routes: /:login/jobs, /:login/machines, /:login/settings
 */
export function InstallationLayout() {
    return (
        <ProtectedInstallationRoute>
            <div>
                <Navigation />
                <InstallationTypeWarning />
                <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6">
                    <Outlet />
                </div>
            </div>
        </ProtectedInstallationRoute>
    )
}
