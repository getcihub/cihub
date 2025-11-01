import { useAuth } from '../hooks/useAuth'

export function UserAvatar() {
    const { user, isAuthenticated } = useAuth()

    // If not authenticated, don't show anything
    if (!isAuthenticated || !user) {
        return null
    }

    return (
        <div className="flex items-center gap-3">
            {/* Avatar Image */}
            <img
                src={user.avatar}
                alt={user.login}
                className="size-10 rounded-full border-2 border-gray-200 object-cover"
            />

            {/* User Info */}
            <div>
                <p className="text-sm font-medium text-gray-900">{user.login}</p>
                <p className="text-xs text-gray-500">{user.email}</p>
            </div>

            {/* Admin Badge (if admin) */}
            {user.admin && (
                <span className="ml-auto rounded-full bg-blue-100 px-3 py-1 text-xs font-semibold text-blue-800">
                    Admin
                </span>
            )}
        </div>
    )
}
