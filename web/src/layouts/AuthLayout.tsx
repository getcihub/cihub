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
            <div className="min-h-screen bg-[#050507] grid-bg">
                <Navigation />
                <Outlet />
            </div>
        </ProtectedRoute>
    );
}
