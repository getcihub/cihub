import { useNavigate } from '@tanstack/react-router'
import { useQueryClient } from '@tanstack/react-query'
import { AuthContext, type AuthProviderProps } from '@/context/AuthContext'
import { useUser } from '@/hooks/useUser'

export function AuthProvider({ children }: AuthProviderProps) {
    const navigate = useNavigate()
    const queryClient = useQueryClient()
    const { data: user = null, isLoading } = useUser()

    const checkAuth = async () => {
        await queryClient.invalidateQueries({ queryKey: ['user'] })
    }

    const logout = async () => {
        try {
            await fetch('/auth/logout', { method: 'POST' })
        } catch (error) {
            console.error('Failed to logout:', error)
        } finally {
            queryClient.setQueryData(['user'], null)
            navigate({ to: '/login' })
        }
    }

    return (
        <AuthContext.Provider
            value={{
                user,
                isLoading,
                isAuthenticated: user !== null,
                logout,
                checkAuth,
            }}
        >
            {children}
        </AuthContext.Provider>
    )
}
