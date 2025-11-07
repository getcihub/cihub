import { Link, useLocation, useNavigate } from '@tanstack/react-router'
import { TabNavigation, TabNavigationLink } from "@/components/TabNavigation"
// import { Notifications } from "./Notifications"
import { DropdownUserProfile } from "./UserProfile"
import { InstallationSwitcher } from "./InstallationSwitcher"
import { useInstallation } from '@/hooks/useInstallation'

function Navigation() {
    const location = useLocation()
    const pathname = location.pathname
    const navigate = useNavigate()
    const { selectedInstallation } = useInstallation()

    // Only show installation-specific navigation if an installation is selected
    const showInstallationNav = selectedInstallation && pathname !== '/' && pathname !== '/account'

    return (
        <div className="shadow-s sticky top-0 z-20 bg-white">
            <div className="mx-auto flex max-w-7xl items-center justify-between px-4 sm:px-6 pt-3">
                <div className="flex items-center gap-6">
                    {/* App Logo */}
                    <button
                        onClick={() => navigate({ to: '/' })}
                        className="text-xl font-bold text-gray-900 hover:text-gray-700 cursor-pointer transition-colors"
                    >
                        CIHub
                    </button>
                    {/* Divider */}
                    <div className="h-6 w-px bg-gray-200" />
                    {/* Installation Switcher */}
                    <InstallationSwitcher />
                </div>
                <div className="flex h-[42px] flex-nowrap gap-1">
                    <DropdownUserProfile />
                </div>
            </div>
            {showInstallationNav && selectedInstallation && (
                <TabNavigation className="mt-5">
                    <div className="mx-auto flex w-full max-w-7xl items-center px-6">
                        <TabNavigationLink
                            className="inline-flex gap-2"
                            asChild
                            active={pathname.includes('/machines')}
                        >
                            <Link to="/$login/machines" params={{ login: selectedInstallation.login }}>Machines</Link>
                        </TabNavigationLink>
                        <TabNavigationLink
                            className="inline-flex gap-2"
                            asChild
                            active={pathname.includes('/jobs')}
                        >
                            <Link to="/$login/jobs" params={{ login: selectedInstallation.login }}>Jobs</Link>
                        </TabNavigationLink>
                        <TabNavigationLink
                            className="inline-flex gap-2"
                            asChild
                            active={pathname.endsWith('/settings')}
                        >
                            <Link to="/$login/settings" params={{ login: selectedInstallation.login }}>Settings</Link>
                        </TabNavigationLink>
                    </div>
                </TabNavigation>
            )}
        </div>
    )
}

export { Navigation }
