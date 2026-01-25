import { useInstallation } from '@/hooks/useInstallation';
import { Link, useLocation, useNavigate } from '@tanstack/react-router';

import { InstallationSwitcher } from './InstallationSwitcher';
import { DropdownUserProfile } from './UserProfile';

function Navigation() {
    const location = useLocation();
    const pathname = location.pathname;
    const navigate = useNavigate();
    const { selectedInstallation } = useInstallation();

    // Only show installation-specific navigation if an installation is selected
    const showInstallationNav =
        selectedInstallation && pathname !== '/' && pathname !== '/account';

    return (
        <div className="border-b border-white/10 bg-transparent">
            <div className="mx-auto flex max-w-7xl items-center justify-between px-4 pt-3 sm:px-6">
                <div className="flex items-center gap-6">
                    {/* App Logo */}
                    <button
                        onClick={() => navigate({ to: '/' })}
                        className="flex cursor-pointer items-center gap-2 font-display text-xl text-white transition-colors hover:text-white/80"
                    >
                        <img
                            src="/favicon.svg"
                            alt="CIHub"
                            className="size-6 invert"
                        />
                        CIHub
                    </button>
                    {/* Divider */}
                    {selectedInstallation ? (
                        <div className="h-6 w-px bg-white/10" />
                    ) : null}
                    {/* Installation Switcher */}
                    <InstallationSwitcher />
                </div>
                <div className="flex h-[42px] flex-nowrap gap-1">
                    <DropdownUserProfile />
                </div>
            </div>
            {showInstallationNav && selectedInstallation && (
                <nav className="mt-4">
                    <div className="mx-auto flex w-full max-w-7xl items-center gap-1 px-6">
                        <Link
                            to="/$login"
                            params={{ login: selectedInstallation.login }}
                            className={`relative flex items-center px-3 pb-3 font-mono text-sm transition-colors ${
                                pathname === `/${selectedInstallation.login}` || pathname === `/${selectedInstallation.login}/`
                                    ? 'text-white'
                                    : 'text-white/50 hover:text-white/70'
                            }`}
                        >
                            Overview
                            {(pathname === `/${selectedInstallation.login}` || pathname === `/${selectedInstallation.login}/`) && (
                                <span className="absolute bottom-0 left-0 right-0 h-px bg-white" />
                            )}
                        </Link>
                        <Link
                            to="/$login/machines"
                            params={{ login: selectedInstallation.login }}
                            className={`relative flex items-center px-3 pb-3 font-mono text-sm transition-colors ${
                                pathname.includes('/machines')
                                    ? 'text-white'
                                    : 'text-white/50 hover:text-white/70'
                            }`}
                        >
                            Machines
                            {pathname.includes('/machines') && (
                                <span className="absolute bottom-0 left-0 right-0 h-px bg-white" />
                            )}
                        </Link>
                        <Link
                            to="/$login/runners"
                            params={{ login: selectedInstallation.login }}
                            className={`relative flex items-center px-3 pb-3 font-mono text-sm transition-colors ${
                                pathname.includes('/runners')
                                    ? 'text-white'
                                    : 'text-white/50 hover:text-white/70'
                            }`}
                        >
                            Runners
                            {pathname.includes('/runners') && (
                                <span className="absolute bottom-0 left-0 right-0 h-px bg-white" />
                            )}
                        </Link>
                        <Link
                            to="/$login/settings"
                            params={{ login: selectedInstallation.login }}
                            className={`relative flex items-center px-3 pb-3 font-mono text-sm transition-colors ${
                                pathname.endsWith('/settings')
                                    ? 'text-white'
                                    : 'text-white/50 hover:text-white/70'
                            }`}
                        >
                            Settings
                            {pathname.endsWith('/settings') && (
                                <span className="absolute bottom-0 left-0 right-0 h-px bg-white" />
                            )}
                        </Link>
                    </div>
                </nav>
            )}
        </div>
    );
}

export { Navigation };
