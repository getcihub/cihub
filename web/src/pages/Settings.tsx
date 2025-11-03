import { useState } from 'react'
import { RiShieldLine, RiCheckLine, RiBillLine, RiSettingsLine, RiArrowRightLine } from '@remixicon/react'
import { useInstallation } from '../hooks/useInstallation'
import { useUsageMetrics } from '../hooks/useUsageMetrics'
import { getPlanConfig } from '../config/plans'
import { Card } from '../components/Card'
import { Button } from '../components/Button'
import { UserEmails } from '../components/UserEmails'

type SettingsSection = 'general' | 'billing'

interface MenuItemProps {
    id: SettingsSection
    label: string
    icon: React.ReactNode
}

export function SettingsPage() {
    const { selectedInstallation } = useInstallation()
    const { machines_used, vcpu_used } = useUsageMetrics()
    const [activeSection, setActiveSection] = useState<SettingsSection>('general')
    const [showConfirmDisconnect, setShowConfirmDisconnect] = useState(false)

    if (!selectedInstallation) {
        return (
            <main>
                <p className="text-gray-600">No installation selected</p>
            </main>
        )
    }

    const handleDisconnectInstallation = () => {
        console.log('Disconnecting installation...')
        setShowConfirmDisconnect(false)
    }

    const menuItems: MenuItemProps[] = [
        { id: 'general', label: 'General', icon: <RiSettingsLine className="size-4" /> },
        { id: 'billing', label: 'Billing', icon: <RiBillLine className="size-4" /> },
    ]

    return (
        <main>
            {/* Settings Layout */}
            <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
                {/* Sidebar Menu */}
                <aside className="lg:col-span-1">
                    <nav className="space-y-1 sticky top-6">
                        {menuItems.map((item) => (
                            <button
                                key={item.id}
                                onClick={() => setActiveSection(item.id)}
                                className={`w-full flex items-center gap-3 px-4 py-3 rounded-lg font-medium text-sm transition-all ${
                                    activeSection === item.id
                                        ? 'bg-blue-50 text-blue-700 border-l-4 border-blue-600'
                                        : 'text-gray-700 hover:bg-gray-50 border-l-4 border-transparent'
                                }`}
                            >
                                {item.icon}
                                <span>{item.label}</span>
                            </button>
                        ))}
                    </nav>
                </aside>

                {/* Main Content */}
                <div className="lg:col-span-3">
                    {/* General Section */}
                    {activeSection === 'general' && (
                        <div className="space-y-6">
                            <div>
                                <h2 className="text-2xl font-semibold text-gray-900">General Settings</h2>
                                <p className="text-gray-600 text-sm mt-1">Manage installation information and contact preferences</p>
                            </div>
                            <div>

                                {/* Installation Overview */}
                                <Card className="p-6 mb-6">
                                    <h3 className="text-lg font-semibold text-gray-900 mb-4">Installation Overview</h3>
                                    <div className="flex items-start gap-6">
                                        <img
                                            src={selectedInstallation.avatar_url}
                                            alt={selectedInstallation.login}
                                            className="size-20 rounded-lg object-cover flex-shrink-0"
                                        />
                                        <div className="flex-1">
                                            <h4 className="text-lg font-semibold text-gray-900">{selectedInstallation.login}</h4>
                                            <p className="text-sm text-gray-600 mt-2">
                                                Type: <span className="font-medium capitalize">{selectedInstallation.account_type}</span>
                                            </p>
                                            {selectedInstallation.membership && (
                                                <p className="text-sm text-gray-600 mt-1">
                                                    Role: <span className="font-medium capitalize">{selectedInstallation.membership.role}</span>
                                                </p>
                                            )}
                                            <p className="text-sm text-gray-600 mt-1">
                                                Created: <span className="font-medium">
                                                    {new Date(selectedInstallation.created_at * 1000).toLocaleDateString('en-US', {
                                                        year: 'numeric',
                                                        month: 'long',
                                                        day: 'numeric',
                                                    })}
                                                </span>
                                            </p>
                                            {selectedInstallation.suspended_at !== 0 && (
                                                <div className="mt-3 inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-red-50 border border-red-200">
                                                    <RiShieldLine className="size-4 text-red-600" />
                                                    <span className="text-xs font-medium text-red-700">Suspended</span>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </Card>

                                {/* Email Addresses */}
                                <UserEmails />


                                {/* Notification Preferences */}
                                <Card className="p-6 mb-6">
                                    <h3 className="text-lg font-semibold text-gray-900 mb-4">Notification Preferences</h3>
                                    <div className="space-y-4">
                                        {[
                                            { title: 'Job Notifications', description: 'Receive alerts when jobs are created, started, or completed' },
                                            { title: 'Runner Alerts', description: 'Get notified when runners go offline or encounter issues' },
                                            { title: 'Billing Updates', description: 'Receive information about billing, invoices, and plan changes' },
                                            { title: 'Security Alerts', description: 'Important notifications about API keys and account security' },
                                        ].map((pref, idx) => (
                                            <div key={idx} className="flex items-center justify-between p-4 bg-gray-50 rounded-lg border border-gray-200">
                                                <div>
                                                    <p className="text-sm font-medium text-gray-900">{pref.title}</p>
                                                    <p className="text-xs text-gray-600 mt-1">{pref.description}</p>
                                                </div>
                                                <input
                                                    type="checkbox"
                                                    defaultChecked={true}
                                                    className="size-5 rounded border-gray-300 text-blue-600 focus:ring-blue-500 flex-shrink-0"
                                                />
                                            </div>
                                        ))}
                                    </div>
                                    <Button className="mt-4 gap-2">
                                        <RiCheckLine className="size-4" />
                                        Save Preferences
                                    </Button>
                                </Card>

                                {/* Danger Zone */}
                                <Card className="p-6 border-red-200 bg-red-50 mt-6">
                                    <h3 className="text-lg font-semibold text-red-900 mb-4">Danger Zone</h3>
                                    <div className="space-y-4">
                                        <div>
                                            <h4 className="text-sm font-semibold text-red-900">Disconnect Installation</h4>
                                            <p className="text-sm text-red-800 mt-1">
                                                This will disconnect the {selectedInstallation.login} installation. All associated runners and jobs will be lost.
                                                This action cannot be undone.
                                            </p>
                                        </div>
                                        {showConfirmDisconnect ? (
                                            <div className="space-y-3 p-4 bg-white border border-red-300 rounded-lg">
                                                <p className="text-sm font-medium text-red-900">
                                                    Are you sure you want to disconnect this installation?
                                                </p>
                                                <div className="flex gap-2">
                                                    <Button
                                                        onClick={handleDisconnectInstallation}
                                                        className="bg-red-600 hover:bg-red-700 text-white gap-2"
                                                    >
                                                        <RiShieldLine className="size-4" />
                                                        Yes, Disconnect
                                                    </Button>
                                                    <Button
                                                        onClick={() => setShowConfirmDisconnect(false)}
                                                        variant="secondary"
                                                    >
                                                        Cancel
                                                    </Button>
                                                </div>
                                            </div>
                                        ) : (
                                            <Button
                                                onClick={() => setShowConfirmDisconnect(true)}
                                                className="bg-red-600 hover:bg-red-700 text-white gap-2"
                                            >
                                                <RiShieldLine className="size-4" />
                                                Disconnect Installation
                                            </Button>
                                        )}
                                    </div>
                                </Card>
                            </div>
                        </div>
                    )}

                    {/* Billing Section */}
                    {activeSection === 'billing' && (
                        <div className="space-y-6">
                            <div>
                                <h2 className="text-2xl font-semibold text-gray-900">Billing & Usage</h2>
                                <p className="text-gray-600 text-sm mt-1">Monitor usage and manage billing information</p>
                            </div>
                            <div>
                                {(() => {
                                    const plan = getPlanConfig(selectedInstallation?.stripe_product_id)
                                    const isFree = plan.id === 'Free'
                                    const machinesPercent = Math.round((machines_used / plan.max_machines) * 100)
                                    const vcpuPercent = Math.round((vcpu_used / plan.max_vcpu) * 100)

                                    const getMachinesBarColor = () => {
                                        if (machinesPercent >= 90) return 'bg-red-600'
                                        if (machinesPercent >= 70) return 'bg-yellow-600'
                                        return 'bg-green-600'
                                    }

                                    const getVcpuBarColor = () => {
                                        if (vcpuPercent >= 90) return 'bg-red-600'
                                        if (vcpuPercent >= 70) return 'bg-yellow-600'
                                        return 'bg-green-600'
                                    }

                                    return (
                                        <>
                                            {/* Current Plan */}
                                            <Card className={`p-6 mb-6 ${isFree ? 'bg-gray-50 border-gray-200' : 'bg-blue-50 border-blue-200'}`}>
                                                <h3 className="text-lg font-semibold text-gray-900 mb-4">Current Plan</h3>
                                                <div className="space-y-4">
                                                    <div className={`flex items-center justify-between p-4 rounded-lg border ${isFree ? 'bg-white border-gray-200' : 'bg-blue-100 border-blue-300'}`}>
                                                        <div>
                                                            <p className="text-sm font-medium text-gray-900">{plan.name} Plan</p>
                                                            {isFree ? (
                                                                <p className="text-xs text-gray-600 mt-1">Free forever • Upgrade anytime</p>
                                                            ) : (
                                                                <p className="text-xs text-gray-600 mt-1">${plan.price}/month • Renews on Jan 1, 2025</p>
                                                            )}
                                                        </div>
                                                        <Button
                                                            variant={isFree ? 'primary' : 'secondary'}
                                                            className={`text-xs flex items-center gap-2 ${isFree ? 'gap-1' : ''}`}
                                                        >
                                                            {isFree ? (
                                                                <>
                                                                    {plan.cta}
                                                                    <RiArrowRightLine className="size-3" />
                                                                </>
                                                            ) : (
                                                                'Change Plan'
                                                            )}
                                                        </Button>
                                                    </div>
                                                </div>
                                            </Card>

                                            {/* Usage */}
                                            <Card className="p-6 mb-6">
                                                <h3 className="text-lg font-semibold text-gray-900 mb-4">Current Usage</h3>
                                                <div className="space-y-6">
                                                    {/* Machines */}
                                                    <div>
                                                        <div className="flex items-center justify-between mb-2">
                                                            <p className="text-sm text-gray-600">Active Machines</p>
                                                            <p className="text-sm font-medium text-gray-900">{machines_used} / {plan.max_machines}</p>
                                                        </div>
                                                        <div className="h-2 bg-gray-200 rounded-full overflow-hidden">
                                                            <div
                                                                className={`h-full transition-all ${getMachinesBarColor()}`}
                                                                style={{ width: `${Math.min(machinesPercent, 100)}%` }}
                                                            />
                                                        </div>
                                                    </div>

                                                    {/* vCPU */}
                                                    <div>
                                                        <div className="flex items-center justify-between mb-2">
                                                            <p className="text-sm text-gray-600">Total vCPU Used</p>
                                                            <p className="text-sm font-medium text-gray-900">{vcpu_used} / {plan.max_vcpu}</p>
                                                        </div>
                                                        <div className="h-2 bg-gray-200 rounded-full overflow-hidden">
                                                            <div
                                                                className={`h-full transition-all ${getVcpuBarColor()}`}
                                                                style={{ width: `${Math.min(vcpuPercent, 100)}%` }}
                                                            />
                                                        </div>
                                                    </div>
                                                </div>
                                            </Card>
                                        </>
                                    )
                                })()}

                                {/* Invoices */}
                                {(() => {
                                    const plan = getPlanConfig(selectedInstallation?.stripe_product_id)
                                    const isFree = plan.id === 'Free'

                                    return (
                                        <Card className="p-6">
                                            <h3 className="text-lg font-semibold text-gray-900 mb-4">Recent Invoices</h3>
                                            {isFree ? (
                                                <div className="text-center py-8">
                                                    <p className="text-gray-600 mb-2">No invoices yet</p>
                                                    <p className="text-sm text-gray-500">Upgrade to a paid plan to start receiving invoices</p>
                                                </div>
                                            ) : (
                                                <div className="space-y-3">
                                                    {[
                                                        { date: 'Dec 1, 2024', amount: '$29.00', status: 'paid' },
                                                        { date: 'Nov 1, 2024', amount: '$29.00', status: 'paid' },
                                                        { date: 'Oct 1, 2024', amount: '$29.00', status: 'paid' },
                                                    ].map((invoice, idx) => (
                                                        <div key={idx} className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
                                                            <div>
                                                                <p className="text-sm font-medium text-gray-900">{invoice.date}</p>
                                                                <p className="text-xs text-gray-500 capitalize">{invoice.status}</p>
                                                            </div>
                                                            <div className="text-right">
                                                                <p className="text-sm font-medium text-gray-900">{invoice.amount}</p>
                                                                <button className="text-xs text-blue-600 hover:text-blue-700 mt-1">Download</button>
                                                            </div>
                                                        </div>
                                                    ))}
                                                </div>
                                            )}
                                        </Card>
                                    )
                                })()}
                            </div>
                        </div>
                    )}

                </div>
            </div>
        </main>
    )
}
