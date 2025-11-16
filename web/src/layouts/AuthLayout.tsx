import { ProtectedRoute } from '@/components/ProtectedRoute';
import { Navigation } from '@/components/ui/Navigation';
import { Outlet } from '@tanstack/react-router';

/**
 * AuthLayout wraps authenticated routes that don't require installation context
 * Routes: /, /account
 */
export function AuthLayout() {
    return (
        <ProtectedRoute>
            <div>
                <Navigation />
                <div className="mx-auto max-w-7xl px-4 py-10 sm:px-6">
                    <Outlet />
                </div>
            </div>
        </ProtectedRoute>
    );
}
