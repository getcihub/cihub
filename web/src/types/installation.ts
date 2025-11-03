export const InstallationTypeOrganization = 'organization'
export const InstallationTypeUser = 'user'

export interface Installation {
    id: number
    login: string
    avatar_url: string
    account_type: typeof InstallationTypeOrganization | typeof InstallationTypeUser
    membership?: Membership
    created_at: number
    suspended_at?: number
    updated_at: number
    stripe_product_id?: string
}

export interface Membership {
    role: string
    state: string
}

export const MembershipRoleAdmin = 'admin'
export const MembershipRoleMember = 'member'
export const MembershipRoleOwner = 'owner'
