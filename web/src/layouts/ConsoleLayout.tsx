import { Outlet } from '@tanstack/react-router'
import { ProtectedRoute } from '../components/ProtectedRoute'
import { Navigation } from '../components/ui/Navigation'

export function ConsoleLayout() {
    return (
        <ProtectedRoute>
            <div>
                <Navigation />
                <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6">
                    <Outlet />
                </div>
            </div>
        </ProtectedRoute>
    )
}
