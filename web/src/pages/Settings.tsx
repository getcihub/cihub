import { useInstallation } from '@/hooks/useInstallation';
import { useVarz } from '@/hooks/useVarz';
import {
    RiExternalLinkLine,
    RiSettingsLine,
    RiShieldLine,
} from '@remixicon/react';
import { motion } from 'framer-motion';
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
                <p className="font-mono text-sm text-white/50">
                    No installation selected
                </p>
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
            <div className="grid grid-cols-1 gap-6 lg:grid-cols-4">
                {/* Sidebar Menu */}
                <motion.aside
                    className="lg:col-span-1"
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ duration: 0.3 }}
                >
                    <nav className="sticky top-6 space-y-1">
                        {menuItems.map((item) => (
                            <button
                                key={item.id}
                                onClick={() => setActiveSection(item.id)}
                                className={`flex w-full items-center gap-3 rounded-lg border-l-4 px-4 py-3 font-mono text-sm transition-all ${
                                    activeSection === item.id
                                        ? 'border-white bg-white/10 text-white'
                                        : 'border-transparent text-white/50 hover:border-white/20 hover:bg-white/5 hover:text-white/70'
                                }`}
                            >
                                {item.icon}
                                <span>{item.label}</span>
                            </button>
                        ))}
                    </nav>
                </motion.aside>

                {/* Main Content */}
                <div className="lg:col-span-3">
                    {/* General Section */}
                    {activeSection === 'general' && (
                        <motion.div
                            className="space-y-6"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.1 }}
                        >
                            <div>
                                <h2 className="font-display text-2xl text-white">
                                    General Settings
                                </h2>
                                <p className="mt-1 font-mono text-sm text-white/50">
                                    Manage installation information and contact
                                    preferences
                                </p>
                            </div>
                            <div>
                                {/* Installation Overview */}
                                <motion.div
                                    className="mb-6 rounded-xl border border-white/10 bg-white/[0.02] p-6"
                                    initial={{ opacity: 0, y: 10 }}
                                    animate={{ opacity: 1, y: 0 }}
                                    transition={{ duration: 0.3, delay: 0.2 }}
                                >
                                    <h3 className="mb-4 font-display text-lg text-white">
                                        Installation Overview
                                    </h3>
                                    <div className="flex items-start gap-6">
                                        <img
                                            src={selectedInstallation.avatar_url}
                                            alt={selectedInstallation.login}
                                            className="size-20 flex-shrink-0 rounded-lg border border-white/10 object-cover"
                                        />
                                        <div className="flex-1">
                                            <h4 className="font-display text-lg text-white">
                                                {selectedInstallation.login}
                                            </h4>
                                            <p className="mt-2 font-mono text-sm text-white/50">
                                                Type:{' '}
                                                <span className="capitalize text-white/70">
                                                    {selectedInstallation.account_type}
                                                </span>
                                            </p>
                                            {selectedInstallation.membership && (
                                                <p className="mt-1 font-mono text-sm text-white/50">
                                                    Role:{' '}
                                                    <span className="capitalize text-white/70">
                                                        {selectedInstallation.membership.role}
                                                    </span>
                                                </p>
                                            )}
                                            <p className="mt-1 font-mono text-sm text-white/50">
                                                Created:{' '}
                                                <span className="text-white/70">
                                                    {new Date(
                                                        selectedInstallation.created_at * 1000,
                                                    ).toLocaleDateString('en-US', {
                                                        year: 'numeric',
                                                        month: 'long',
                                                        day: 'numeric',
                                                    })}
                                                </span>
                                            </p>
                                            {selectedInstallation.suspended_at !== 0 && (
                                                <div className="mt-3 inline-flex items-center gap-2 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-1.5">
                                                    <RiShieldLine className="size-4 text-red-400" />
                                                    <span className="font-mono text-xs text-red-400">
                                                        Suspended
                                                    </span>
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                </motion.div>

                                {/* Installation Settings */}
                                <motion.div
                                    className="mb-6 rounded-xl border border-white/10 bg-white/[0.02] p-6"
                                    initial={{ opacity: 0, y: 10 }}
                                    animate={{ opacity: 1, y: 0 }}
                                    transition={{ duration: 0.3, delay: 0.3 }}
                                >
                                    <h3 className="mb-4 font-display text-lg text-white">
                                        Installation Settings
                                    </h3>
                                    <p className="mb-6 font-mono text-sm text-white/50">
                                        Manage your GitHub App installation settings,
                                        permissions, and repository access on GitHub.
                                    </p>
                                    <button
                                        onClick={handleEditInstallation}
                                        className="inline-flex items-center gap-2 rounded-lg bg-white px-4 py-2 font-mono text-sm text-black transition-colors hover:bg-white/90"
                                    >
                                        <RiSettingsLine className="size-4" />
                                        Edit on GitHub
                                        <RiExternalLinkLine className="size-4" />
                                    </button>
                                </motion.div>
                            </div>
                        </motion.div>
                    )}
                </div>
            </div>
        </main>
    );
}
