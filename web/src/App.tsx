import { Outlet } from '@tanstack/react-router'
import { Toaster } from 'sonner'
import { AuthProvider } from './providers/AuthProvider'
import { InstallationProvider } from './providers/InstallationProvider'

function App() {
    return (
        <AuthProvider>
            <InstallationProvider>
                <Outlet />
                <Toaster />
            </InstallationProvider>
        </AuthProvider>
    )
}

export default App
