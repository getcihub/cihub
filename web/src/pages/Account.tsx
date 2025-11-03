import { useNavigate } from '@tanstack/react-router'
import { useAuth } from '../hooks/useAuth'
import { UserEmails } from '../components/UserEmails'
// import { Card } from '../components/Card'
import { Card } from '../components/ui/card'
import { Button } from '../components/Button'

export function AccountPage() {
    const navigate = useNavigate()
    const { user, logout } = useAuth()

    const handleLogout = async () => {
        await logout()
        navigate({ to: '/login' })
    }

    if (!user) {
        return (
            <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
                <p className="text-gray-600">Loading...</p>
            </div>
        )
    }

    return (
        <div className="mx-auto max-w-2xl px-4 py-10 sm:px-6">
            <div className="mb-8">
                <h1 className="text-3xl font-bold text-gray-900">Account Settings</h1>
                <p className="text-gray-600 mt-2">Manage your account and preferences</p>
            </div>

            {/* Profile Section */}
            <Card className="p-6 mb-6">
                <h2 className="text-xl font-semibold text-gray-900 mb-6">Profile</h2>
                <div className="flex items-center gap-6 mb-8">
                    <img
                        src={user.avatar_url}
                        alt={user.login}
                        className="size-16 rounded-full border-2 border-gray-200 object-cover"
                    />
                    <div>
                        <h3 className="text-lg font-medium text-gray-900">{user.login}</h3>
                        <p className="text-sm text-gray-500 mt-1">{user.email}</p>
                        <p className="text-xs text-gray-400 mt-2">
                            Member since {new Date(user.created_at * 1000).toLocaleDateString()}
                        </p>
                    </div>
                </div>
            </Card>

            {/* Email Preferences Section */}
            <UserEmails />

            {/* Logout Section */}
            <Card className="p-6 border-red-200 bg-red-50">
                <h2 className="text-xl font-semibold text-gray-900 mb-4">Session</h2>
                <p className="text-gray-600 text-sm mb-4">
                    Sign out from your account. You'll need to authenticate again to access this app.
                </p>
                <Button onClick={handleLogout} variant="destructive">
                    Sign Out
                </Button>
            </Card>
        </div>
    )
}
