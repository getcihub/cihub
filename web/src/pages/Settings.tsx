import { Button } from '@/components/Button';
import { Card } from '@/components/Card';
import { useInstallation } from '@/hooks/useInstallation';
import { useVarz } from '@/hooks/useVarz';
import {
    RiExternalLinkLine,
    RiSettingsLine,
    RiShieldLine,
} from '@remixicon/react';
import { useState } from 'react';

type SettingsSection = 'general' | 'billing';

interface MenuItemProps {
    id: SettingsSection;
    label: string;
    icon: React.ReactNode;
}

export function SettingsPage() {
    const { selectedInstallation } = useInstallation();
    const { data: varz } = useVarz();
    const [activeSection, setActiveSection] =
        useState<SettingsSection>('general');

    if (!selectedInstallation) {
        return (
            <main>
                <p className="text-gray-600">No installation selected</p>
            </main>
        );
    }

    const handleEditInstallation = () => {
        if (varz?.github?.name && selectedInstallation.id) {
            window.location.href = `https://github.com/apps/${varz.github.name}/installations/${selectedInstallation.id}`;
        }
    };

    const menuItems: MenuItemProps[] = [
        {
            id: 'general',
            label: 'General',
            icon: <RiSettingsLine className="size-4" />,
        },
    ];

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
                                <h2 className="text-2xl font-semibold text-gray-900">
                                    General Settings
                                </h2>
                                <p className="text-gray-600 text-sm mt-1">
                                    Manage installation information and contact
                                    preferences
                                </p>
                            </div>
                            <div>
                                {/* Installation Overview */}
                                <Card className="p-6 mb-6">
                                    <h3 className="text-lg font-semibold text-gray-900 mb-4">
                                        Installation Overview
                                    </h3>
                                    <div className="flex items-start gap-6">
                                        <img
                                            src={
                                                selectedInstallation.avatar_url
                                            }
                                            alt={selectedInstallation.login}
                                            className="size-20 rounded-lg object-cover flex-shrink-0"
                                        />
                                        <div className="flex-1">
                                            <h4 className="text-lg font-semibold text-gray-900">
                                                {selectedInstallation.login}
                                            </h4>
                                            <p className="text-sm text-gray-600 mt-2">
                                                Type:{' '}
                                                <span className="font-medium capitalize">
                                                    {
                                                        selectedInstallation.account_type
                                                    }
                                                </span>
                                            </p>
                                            {selectedInstallation.membership && (
                                                <p className="text-sm text-gray-600 mt-1">
                                                    Role:{' '}
                                                    <span className="font-medium capitalize">
                                                        {
                                                            selectedInstallation
                                                                .membership.role
                                                        }
                                                    </span>
                                                </p>
                                            )}
                                            <p className="text-sm text-gray-600 mt-1">
                                                Created:{' '}
                                                <span className="font-medium">
                                                    {new Date(
                                                        selectedInstallation.created_at *
                                                            1000,
                                                    ).toLocaleDateString(
                                                        'en-US',
                                                        {
                                                            year: 'numeric',
                                                            month: 'long',
                                                            day: 'numeric',
                                                        },
                                                    )}
                                                </span>
                                            </p>
                                            {selectedInstallation.suspended_at !==
                                                0 && (
                                                <div className="mt-3 inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-red-50 border border-red-200">
                                                    <RiShieldLine className="size-4 text-red-600" />
                                                    <span className="text-xs font-medium text-red-700">
                                                        Suspended
                                                    </span>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </Card>

                                {/* Installation Settings */}
                                <Card className="p-6 mb-6">
                                    <h3 className="text-lg font-semibold text-gray-900 mb-4">
                                        Installation Settings
                                    </h3>
                                    <p className="text-gray-600 text-sm mb-6">
                                        Manage your GitHub App installation
                                        settings, permissions, and repository
                                        access on GitHub.
                                    </p>
                                    <Button
                                        onClick={handleEditInstallation}
                                        className="gap-2"
                                    >
                                        <RiSettingsLine className="size-4" />
                                        Edit on GitHub
                                        <RiExternalLinkLine className="size-4" />
                                    </Button>
                                </Card>
                            </div>
                        </div>
                    )}
                </div>
            </div>
        </main>
    );
}
