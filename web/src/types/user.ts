export interface User {
    id: number
    login: string
    avatar: string
    email: string
    admin: boolean
    active: boolean
    created: number
    updated: number
}

export interface Email {
    email: string
    primary?: boolean
    verified?: boolean
}

export interface ApiResponse<T> {
    error: boolean
    reason: string
    data: T
}
