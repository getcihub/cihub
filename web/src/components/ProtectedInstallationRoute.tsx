import { useNavigate, useParams } from '@tanstack/react-router'
import { useEffect } from 'react'
import { useAuth } from '@/hooks/useAuth'
import { useInstallationDetails } from '@/hooks/useInstallationDetails'
import { InstallationAccessError } from './InstallationAccessError'
import { Skeleton } from './Skeleton'

interface ProtectedInstallationRouteProps {
    children: React.ReactNode
}

/**
 * ProtectedInstallationRoute ensures that:
 * 1. User is authenticated
 * 2. Installation access is verified (not found or unauthorized)
 * 3. An installation is selected (either from context or inferred from URL)
 * Routes like /:login/jobs, /:login/machines, /:login/settings require this protection
 */
export function ProtectedInstallationRoute({ children }: ProtectedInstallationRouteProps) {
    const { isAuthenticated, isLoading: authLoading } = useAuth()
    const { login } = useParams({ from: '/$login' })
    const navigate = useNavigate()
    const { isLoading: detailsLoading, error } = useInstallationDetails(login)

    useEffect(() => {
        // Don't redirect while auth is still loading
        if (authLoading) {
            return
        }

        if (!isAuthenticated) {
            navigate({ to: '/login' })
            return
        }
    }, [authLoading, isAuthenticated, navigate])

    // Show loading state while auth or details are loading
    if (authLoading || detailsLoading) {
        return <Skeleton className="h-screen w-full" />
    }

    // If not authenticated, loading effect will handle redirect
    if (!isAuthenticated) {
        return null
    }

    // Check for installation access errors
    if (error) {
        const status = (error as any).status || 500
        const reason = (error as any).reason || 'unknown_error'
        return <InstallationAccessError status={status} reason={reason} />
    }

    return children
}
