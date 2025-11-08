import { useContext } from 'react'
import { InstallationContext } from '@/context/InstallationContext'

export function useInstallation() {
    const context = useContext(InstallationContext)
    if (!context) {
        throw new Error('useInstallation must be used within an InstallationProvider')
    }
    return context
}
