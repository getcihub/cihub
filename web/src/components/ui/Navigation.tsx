import { Link, useLocation } from '@tanstack/react-router'
import { TabNavigation, TabNavigationLink } from "../TabNavigation"
// import { Notifications } from "./Notifications"
import { DropdownUserProfile } from "./UserProfile"

function Navigation() {
    const location = useLocation()
    const pathname = location.pathname

    return (
        <div className="shadow-s sticky top-0 z-20 bg-white">
            <div className="mx-auto flex max-w-7xl items-center justify-between px-4 sm:px-6 pt-3">
                <div>
                    <span className="sr-only">CIHub</span>
                </div>
                <div className="flex h-[42px] flex-nowrap gap-1">
                    <DropdownUserProfile />
                </div>
            </div>
            <TabNavigation className="mt-5">
                <div className="mx-auto flex w-full max-w-7xl items-center px-6">
                    <TabNavigationLink
                        className="inline-flex gap-2"
                        asChild
                        active={pathname === "/"}
                    >
                        <Link to="/">Jobs</Link>
                    </TabNavigationLink>
                    <TabNavigationLink
                        className="inline-flex gap-2"
                        asChild
                        active={pathname === "/machines"}
                    >
                        <Link to="/machines">Machines</Link>
                    </TabNavigationLink>
                    <TabNavigationLink
                        className="inline-flex gap-2"
                        asChild
                        active={pathname === "/settings"}
                    >
                        <Link to="/settings">Settings</Link>
                    </TabNavigationLink>
                </div>
            </TabNavigation>
        </div>
    )
}

export { Navigation }
