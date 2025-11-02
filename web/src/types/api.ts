export interface ApiResponse<T> {
    error: boolean
    reason: string
    data: T
}

export interface PaginatedApiResponse<T> {
    error: boolean
    reason: string
    has_more: boolean
    data: T[]
}
