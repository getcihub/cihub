import type { ReactNode } from 'react'
import { createContext } from 'react'
import type { User } from '@/types/user'

export interface AuthContextType {
    user: User | null
    isLoading: boolean
    isAuthenticated: boolean
    logout: () => void
    checkAuth: () => Promise<void>
}

export const AuthContext = createContext<AuthContextType | undefined>(undefined)

export interface AuthProviderProps {
    children: ReactNode
}
