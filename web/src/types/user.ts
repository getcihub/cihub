export interface User {
    id: number
    login: string
    avatar_url: string
    email: string
    admin: boolean
    active: boolean
    created_at: number
    updated_at: number
    synced_at?: number
    syncing?: boolean
}

export interface UserEmail {
    email: string
    primary?: boolean
    verified?: boolean
}
