import { useEffect, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { InstallationContext, type InstallationProviderProps } from '@/context/InstallationContext'
import type { Installation, Membership } from '@/types/installation'
import type { ApiResponse } from '@/types/api'

const STORAGE_KEY = 'selectedInstallation'

export function InstallationProvider({ children }: InstallationProviderProps) {
    const queryClient = useQueryClient()
    const [selectedInstallation, setSelectedInstallation] = useState<Installation | null>(null)
    const [membership, setMembership] = useState<Membership | null>(null)
    const [isLoading, setIsLoading] = useState(true)

    // Load selected installation from localStorage on mount
    useEffect(() => {
        const loadInstallation = async () => {
            setIsLoading(true)
            try {
                const stored = localStorage.getItem(STORAGE_KEY)
                if (stored) {
                    const installation = JSON.parse(stored) as Installation
                    // Verify the installation still exists and user has access
                    const response = await fetch(`/api/installations/${installation.login}`)
                    const data = (await response.json()) as ApiResponse<Installation>

                    if (!data.error && data.data) {
                        setSelectedInstallation(data.data)
                        setMembership(data.data.membership || null)
                    } else {
                        // Clear invalid installation
                        localStorage.removeItem(STORAGE_KEY)
                        setSelectedInstallation(null)
                        setMembership(null)
                    }
                } else {
                    setSelectedInstallation(null)
                    setMembership(null)
                }
            } catch (error) {
                console.error('Failed to load installation:', error)
                localStorage.removeItem(STORAGE_KEY)
                setSelectedInstallation(null)
                setMembership(null)
            } finally {
                setIsLoading(false)
            }
        }

        loadInstallation()
    }, [])

    const selectInstallation = async (installation: Installation) => {
        setIsLoading(true)
        try {
            // Fetch installation details to verify access and get membership
            const response = await fetch(`/api/installations/${installation.login}`)
            const data = (await response.json()) as ApiResponse<Installation>

            if (data.error || !data.data) {
                throw new Error(data.reason || 'Failed to select installation')
            }

            setSelectedInstallation(data.data)
            setMembership(data.data.membership || null)
            localStorage.setItem(STORAGE_KEY, JSON.stringify(data.data))

            // Invalidate queries that might depend on installation context
            await queryClient.invalidateQueries()
        } catch (error) {
            console.error('Failed to select installation:', error)
            setSelectedInstallation(null)
            setMembership(null)
            localStorage.removeItem(STORAGE_KEY)
            throw error
        } finally {
            setIsLoading(false)
        }
    }

    const clearInstallation = () => {
        setSelectedInstallation(null)
        setMembership(null)
        localStorage.removeItem(STORAGE_KEY)
        queryClient.clear()
    }

    return (
        <InstallationContext.Provider
            value={{
                selectedInstallation,
                membership,
                isLoading,
                selectInstallation,
                clearInstallation,
            }}
        >
            {children}
        </InstallationContext.Provider>
    )
}
