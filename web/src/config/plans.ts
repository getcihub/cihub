export interface PlanConfig {
    id: string
    name: string
    price: number | null
    billing_period?: string
    max_machines: number
    max_vcpu: number
    description: string
    cta?: string
}

export const PLANS: Record<string, PlanConfig> = {
    'Free': {
        id: 'Free',
        name: 'Free',
        price: null,
        max_machines: 3,
        max_vcpu: 30,
        description: 'Get started with CIHub',
        cta: 'Upgrade to Startup',
    },
    'startup': {
        id: 'startup',
        name: 'Startup',
        price: 29,
        billing_period: 'month',
        max_machines: 10,
        max_vcpu: 200,
        description: 'For growing teams',
    },
}

/**
 * Get plan configuration by Stripe product ID
 * Defaults to Free plan if not found
 */
export function getPlanConfig(stripProductId?: string): PlanConfig {
    if (!stripProductId) {
        return PLANS['Free']
    }
    return PLANS[stripProductId] || PLANS['Free']
}
