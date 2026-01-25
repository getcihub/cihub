import { StatCard } from '@/components/StatCard';
import { useInstallation } from '@/hooks/useInstallation';
import { useMachines } from '@/hooks/useMachines';
import { useRunners } from '@/hooks/useRunners';
import { useVarz } from '@/hooks/useVarz';
import { cx } from '@/lib/utils';
import {
    MembershipRoleAdmin,
    MembershipRoleOwner,
} from '@/types/installation';
import { MachineStatusOnline } from '@/types/machine';
import {
    RunnerStatusBusy,
    RunnerStatusIdle,
} from '@/types/runner';
import {
    RiCheckLine,
    RiComputerLine,
    RiExternalLinkLine,
    RiGithubFill,
    RiGroupLine,
    RiInformationLine,
    RiLink,
    RiPlayCircleLine,
    RiServerLine,
    RiSettings3Line,
    RiShieldCheckLine,
    RiShieldLine,
    RiTimeLine,
} from '@remixicon/react';
import { motion } from 'framer-motion';
import { useState } from 'react';

type SettingsSection = 'overview' | 'permissions' | 'danger';

interface MenuItemProps {
    id: SettingsSection;
    label: string;
    icon: React.ReactNode;
    description: string;
}

export function SettingsPage() {
    const { selectedInstallation } = useInstallation();
    const { data: varz } = useVarz();
    const { data: machines = [] } = useMachines();
    const { data: runners = [] } = useRunners();
    const [activeSection, setActiveSection] =
        useState<SettingsSection>('overview');

    if (!selectedInstallation) {
        return (
            <main className="flex items-center justify-center py-20">
                <div className="text-center">
                    <RiInformationLine className="mx-auto size-12 text-white/30" />
                    <p className="mt-4 font-mono text-sm text-white/50">
                        No installation selected
                    </p>
                </div>
            </main>
        );
    }

    const handleEditInstallation = () => {
        if (varz?.github?.name && selectedInstallation.id) {
            window.open(
                `https://github.com/apps/${varz.github.name}/installations/${selectedInstallation.id}`,
                '_blank'
            );
        }
    };

    const handleViewOnGitHub = () => {
        const url = selectedInstallation.account_type === 'organization'
            ? `https://github.com/${selectedInstallation.login}`
            : `https://github.com/${selectedInstallation.login}`;
        window.open(url, '_blank');
    };

    // Calculate stats
    const activeMachines = machines.filter(m => m.status === MachineStatusOnline).length;
    const activeRunners = runners.filter(r => r.status === RunnerStatusBusy || r.status === RunnerStatusIdle).length;
    const busyRunners = runners.filter(r => r.status === RunnerStatusBusy).length;

    // Check if user has admin role
    const isAdmin = selectedInstallation.membership?.role === MembershipRoleAdmin ||
        selectedInstallation.membership?.role === MembershipRoleOwner;

    const isSuspended = selectedInstallation.suspended_at !== undefined && selectedInstallation.suspended_at !== 0;

    const menuItems: MenuItemProps[] = [
        {
            id: 'overview',
            label: 'Overview',
            icon: <RiInformationLine className="size-4" />,
            description: 'Installation details and stats',
        },
        {
            id: 'permissions',
            label: 'Permissions',
            icon: <RiShieldCheckLine className="size-4" />,
            description: 'GitHub App permissions',
        },
        {
            id: 'danger',
            label: 'Danger Zone',
            icon: <RiShieldLine className="size-4" />,
            description: 'Critical actions',
        },
    ];

    const createdDate = new Date(selectedInstallation.created_at * 1000);
    const daysSinceCreation = Math.floor((Date.now() - createdDate.getTime()) / (1000 * 60 * 60 * 24));

    return (
        <main className="pb-12">
            {/* Header */}
            <motion.div
                className="mb-8"
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
            >
                <div className="flex items-center gap-2 font-mono text-sm text-white/50">
                    <RiSettings3Line className="size-4" />
                    <span>Settings</span>
                </div>
                <h1 className="mt-2 font-display text-3xl text-white">
                    Installation Settings
                </h1>
                <p className="mt-1 font-mono text-sm text-white/50">
                    Manage your {selectedInstallation.login} installation
                </p>
            </motion.div>

            {/* Suspended Banner */}
            {isSuspended && (
                <motion.div
                    className="mb-6 flex items-center gap-3 rounded-xl border border-red-500/30 bg-red-500/10 p-4"
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.3, delay: 0.1 }}
                >
                    <RiShieldLine className="size-5 text-red-400" />
                    <div>
                        <p className="font-mono text-sm text-red-400">
                            This installation is currently suspended
                        </p>
                        <p className="mt-0.5 font-mono text-xs text-red-400/70">
                            Please check your GitHub App settings or contact support
                        </p>
                    </div>
                </motion.div>
            )}

            {/* Settings Layout */}
            <div className="grid grid-cols-1 gap-8 lg:grid-cols-4">
                {/* Sidebar Menu */}
                <motion.aside
                    className="lg:col-span-1"
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ duration: 0.3 }}
                >
                    <nav className="sticky top-6 space-y-1">
                        {menuItems.map((item, index) => (
                            <motion.button
                                key={item.id}
                                onClick={() => setActiveSection(item.id)}
                                className={cx(
                                    'flex w-full items-start gap-3 rounded-lg border px-4 py-3 text-left transition-all',
                                    activeSection === item.id
                                        ? 'border-white/20 bg-white/10 text-white'
                                        : 'border-transparent text-white/50 hover:border-white/10 hover:bg-white/5 hover:text-white/70',
                                    item.id === 'danger' && 'mt-4'
                                )}
                                initial={{ opacity: 0, x: -10 }}
                                animate={{ opacity: 1, x: 0 }}
                                transition={{ duration: 0.2, delay: index * 0.05 }}
                            >
                                <span className={cx(
                                    'mt-0.5',
                                    item.id === 'danger' && activeSection === item.id && 'text-red-400'
                                )}>
                                    {item.icon}
                                </span>
                                <div>
                                    <span className={cx(
                                        'font-mono text-sm',
                                        item.id === 'danger' && 'text-red-400'
                                    )}>
                                        {item.label}
                                    </span>
                                    <p className="mt-0.5 font-mono text-xs text-white/40">
                                        {item.description}
                                    </p>
                                </div>
                            </motion.button>
                        ))}
                    </nav>

                    {/* Quick Actions */}
                    <motion.div
                        className="mt-8 rounded-xl border border-white/10 bg-white/[0.02] p-4"
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ duration: 0.3, delay: 0.3 }}
                    >
                        <h4 className="mb-3 font-mono text-xs uppercase tracking-wider text-white/50">
                            Quick Actions
                        </h4>
                        <div className="space-y-2">
                            <button
                                onClick={handleViewOnGitHub}
                                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 font-mono text-xs text-white/70 transition-colors hover:bg-white/5 hover:text-white"
                            >
                                <RiGithubFill className="size-4" />
                                View on GitHub
                                <RiExternalLinkLine className="ml-auto size-3 text-white/40" />
                            </button>
                            <button
                                onClick={handleEditInstallation}
                                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 font-mono text-xs text-white/70 transition-colors hover:bg-white/5 hover:text-white"
                            >
                                <RiSettings3Line className="size-4" />
                                Edit Installation
                                <RiExternalLinkLine className="ml-auto size-3 text-white/40" />
                            </button>
                        </div>
                    </motion.div>
                </motion.aside>

                {/* Main Content */}
                <div className="lg:col-span-3">
                    {/* Overview Section */}
                    {activeSection === 'overview' && (
                        <motion.div
                            className="space-y-6"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.1 }}
                            key="overview"
                        >
                            {/* Stats Grid */}
                            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
                                <StatCard
                                    label="Machines"
                                    value={machines.length}
                                    subValue={`${activeMachines} active`}
                                    icon={RiServerLine}
                                    iconColor="text-blue-400"
                                    iconBgColor="bg-blue-500/20"
                                    delay={0}
                                />
                                <StatCard
                                    label="Runners"
                                    value={runners.length}
                                    subValue={`${busyRunners} busy`}
                                    icon={RiPlayCircleLine}
                                    iconColor="text-emerald-400"
                                    iconBgColor="bg-emerald-500/20"
                                    delay={0.05}
                                />
                                <StatCard
                                    label="Active Since"
                                    value={daysSinceCreation}
                                    subValue="days"
                                    icon={RiTimeLine}
                                    iconColor="text-purple-400"
                                    iconBgColor="bg-purple-500/20"
                                    delay={0.1}
                                />
                                <StatCard
                                    label="Status"
                                    value={isSuspended ? 'Suspended' : 'Active'}
                                    subValue={isAdmin ? 'Admin access' : 'Member access'}
                                    icon={isSuspended ? RiShieldLine : RiCheckLine}
                                    iconColor={isSuspended ? 'text-red-400' : 'text-emerald-400'}
                                    iconBgColor={isSuspended ? 'bg-red-500/20' : 'bg-emerald-500/20'}
                                    delay={0.15}
                                />
                            </div>

                            {/* Installation Overview Card */}
                            <motion.div
                                className="rounded-xl border border-white/10 bg-white/[0.02] overflow-hidden"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: 0.2 }}
                            >
                                <div className="border-b border-white/10 bg-white/[0.02] px-6 py-4">
                                    <div className="flex items-center gap-2">
                                        <RiComputerLine className="size-5 text-white/70" />
                                        <h3 className="font-display text-lg text-white">
                                            Installation Details
                                        </h3>
                                    </div>
                                </div>
                                <div className="p-6">
                                    <div className="flex items-start gap-6">
                                        <img
                                            src={selectedInstallation.avatar_url}
                                            alt={selectedInstallation.login}
                                            className="size-20 flex-shrink-0 rounded-xl border border-white/10 object-cover"
                                        />
                                        <div className="flex-1 space-y-4">
                                            <div>
                                                <h4 className="font-display text-xl text-white">
                                                    {selectedInstallation.login}
                                                </h4>
                                                <p className="mt-1 font-mono text-sm text-white/50">
                                                    GitHub {selectedInstallation.account_type === 'organization' ? 'Organization' : 'User'} Account
                                                </p>
                                            </div>
                                            <div className="grid grid-cols-2 gap-4">
                                                <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
                                                    <p className="font-mono text-xs text-white/50">Installation ID</p>
                                                    <p className="mt-1 font-mono text-sm text-white">{selectedInstallation.id}</p>
                                                </div>
                                                <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
                                                    <p className="font-mono text-xs text-white/50">Account Type</p>
                                                    <p className="mt-1 font-mono text-sm capitalize text-white">{selectedInstallation.account_type}</p>
                                                </div>
                                                <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
                                                    <p className="font-mono text-xs text-white/50">Your Role</p>
                                                    <p className="mt-1 font-mono text-sm capitalize text-white">
                                                        {selectedInstallation.membership?.role || 'N/A'}
                                                    </p>
                                                </div>
                                                <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
                                                    <p className="font-mono text-xs text-white/50">Created</p>
                                                    <p className="mt-1 font-mono text-sm text-white">
                                                        {createdDate.toLocaleDateString('en-US', {
                                                            year: 'numeric',
                                                            month: 'short',
                                                            day: 'numeric',
                                                        })}
                                                    </p>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </motion.div>

                            {/* Resources Summary */}
                            <motion.div
                                className="rounded-xl border border-white/10 bg-white/[0.02] overflow-hidden"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: 0.3 }}
                            >
                                <div className="border-b border-white/10 bg-white/[0.02] px-6 py-4">
                                    <div className="flex items-center gap-2">
                                        <RiGroupLine className="size-5 text-white/70" />
                                        <h3 className="font-display text-lg text-white">
                                            Resources Summary
                                        </h3>
                                    </div>
                                </div>
                                <div className="p-6">
                                    <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
                                        <div className="flex items-center gap-4 rounded-lg border border-white/10 bg-white/[0.02] p-4">
                                            <div className="rounded-lg bg-blue-500/20 p-2.5">
                                                <RiServerLine className="size-5 text-blue-400" />
                                            </div>
                                            <div>
                                                <p className="font-display text-2xl text-white">{machines.length}</p>
                                                <p className="font-mono text-xs text-white/50">Total Machines</p>
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-4 rounded-lg border border-white/10 bg-white/[0.02] p-4">
                                            <div className="rounded-lg bg-emerald-500/20 p-2.5">
                                                <RiPlayCircleLine className="size-5 text-emerald-400" />
                                            </div>
                                            <div>
                                                <p className="font-display text-2xl text-white">{activeRunners}</p>
                                                <p className="font-mono text-xs text-white/50">Active Runners</p>
                                            </div>
                                        </div>
                                        <div className="flex items-center gap-4 rounded-lg border border-white/10 bg-white/[0.02] p-4">
                                            <div className="rounded-lg bg-amber-500/20 p-2.5">
                                                <RiComputerLine className="size-5 text-amber-400" />
                                            </div>
                                            <div>
                                                <p className="font-display text-2xl text-white">{activeMachines}</p>
                                                <p className="font-mono text-xs text-white/50">Connected Machines</p>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </motion.div>
                        </motion.div>
                    )}

                    {/* Permissions Section */}
                    {activeSection === 'permissions' && (
                        <motion.div
                            className="space-y-6"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.1 }}
                            key="permissions"
                        >
                            {/* Permissions Card */}
                            <motion.div
                                className="rounded-xl border border-white/10 bg-white/[0.02] overflow-hidden"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: 0.2 }}
                            >
                                <div className="border-b border-white/10 bg-white/[0.02] px-6 py-4">
                                    <div className="flex items-center gap-2">
                                        <RiShieldCheckLine className="size-5 text-white/70" />
                                        <h3 className="font-display text-lg text-white">
                                            GitHub App Permissions
                                        </h3>
                                    </div>
                                </div>
                                <div className="p-6">
                                    <p className="mb-6 font-mono text-sm text-white/50">
                                        Manage the permissions and repository access for your GitHub App installation.
                                        Changes to permissions are made through GitHub.
                                    </p>

                                    <div className="space-y-3">
                                        {[
                                            { name: 'Actions', description: 'Workflows and workflow runs', level: 'Read & Write' },
                                            { name: 'Checks', description: 'Check runs and check suites', level: 'Read & Write' },
                                            { name: 'Contents', description: 'Repository contents and files', level: 'Read' },
                                            { name: 'Metadata', description: 'Repository metadata', level: 'Read' },
                                            { name: 'Administration', description: 'Repository administration', level: 'Read & Write' },
                                        ].map((permission, index) => (
                                            <motion.div
                                                key={permission.name}
                                                className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] px-4 py-3"
                                                initial={{ opacity: 0, x: -10 }}
                                                animate={{ opacity: 1, x: 0 }}
                                                transition={{ duration: 0.2, delay: 0.1 + index * 0.05 }}
                                            >
                                                <div>
                                                    <p className="font-mono text-sm text-white">{permission.name}</p>
                                                    <p className="font-mono text-xs text-white/50">{permission.description}</p>
                                                </div>
                                                <span className={cx(
                                                    'rounded-full px-2.5 py-1 font-mono text-xs',
                                                    permission.level === 'Read & Write'
                                                        ? 'bg-emerald-500/20 text-emerald-400'
                                                        : 'bg-blue-500/20 text-blue-400'
                                                )}>
                                                    {permission.level}
                                                </span>
                                            </motion.div>
                                        ))}
                                    </div>

                                    <div className="mt-6 flex items-center gap-3">
                                        <button
                                            onClick={handleEditInstallation}
                                            className="inline-flex items-center gap-2 rounded-lg bg-white px-4 py-2.5 font-mono text-sm text-black transition-colors hover:bg-white/90"
                                        >
                                            <RiSettings3Line className="size-4" />
                                            Manage Permissions
                                            <RiExternalLinkLine className="size-4" />
                                        </button>
                                        <p className="font-mono text-xs text-white/40">
                                            Opens GitHub Settings
                                        </p>
                                    </div>
                                </div>
                            </motion.div>

                            {/* Repository Access Card */}
                            <motion.div
                                className="rounded-xl border border-white/10 bg-white/[0.02] overflow-hidden"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: 0.3 }}
                            >
                                <div className="border-b border-white/10 bg-white/[0.02] px-6 py-4">
                                    <div className="flex items-center gap-2">
                                        <RiLink className="size-5 text-white/70" />
                                        <h3 className="font-display text-lg text-white">
                                            Repository Access
                                        </h3>
                                    </div>
                                </div>
                                <div className="p-6">
                                    <p className="mb-4 font-mono text-sm text-white/50">
                                        Configure which repositories this installation can access.
                                        You can grant access to all repositories or select specific ones.
                                    </p>
                                    <button
                                        onClick={handleEditInstallation}
                                        className="inline-flex items-center gap-2 rounded-lg border border-white/20 bg-white/5 px-4 py-2.5 font-mono text-sm text-white transition-colors hover:bg-white/10"
                                    >
                                        Configure Repository Access
                                        <RiExternalLinkLine className="size-4" />
                                    </button>
                                </div>
                            </motion.div>
                        </motion.div>
                    )}

                    {/* Danger Zone Section */}
                    {activeSection === 'danger' && (
                        <motion.div
                            className="space-y-6"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ duration: 0.3, delay: 0.1 }}
                            key="danger"
                        >
                            {/* Warning Banner */}
                            <motion.div
                                className="flex items-start gap-3 rounded-xl border border-amber-500/30 bg-amber-500/10 p-4"
                                initial={{ opacity: 0, y: -10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: 0.15 }}
                            >
                                <RiShieldLine className="mt-0.5 size-5 text-amber-400" />
                                <div>
                                    <p className="font-mono text-sm text-amber-400">
                                        Danger Zone
                                    </p>
                                    <p className="mt-0.5 font-mono text-xs text-amber-400/70">
                                        Actions in this section can have significant consequences.
                                        Please proceed with caution.
                                    </p>
                                </div>
                            </motion.div>

                            {/* Uninstall Card */}
                            <motion.div
                                className="rounded-xl border border-red-500/30 bg-red-500/5 overflow-hidden"
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ duration: 0.3, delay: 0.2 }}
                            >
                                <div className="border-b border-red-500/20 bg-red-500/10 px-6 py-4">
                                    <div className="flex items-center gap-2">
                                        <RiShieldLine className="size-5 text-red-400" />
                                        <h3 className="font-display text-lg text-red-400">
                                            Uninstall Application
                                        </h3>
                                    </div>
                                </div>
                                <div className="p-6">
                                    <p className="mb-4 font-mono text-sm text-white/70">
                                        Removing this installation will:
                                    </p>
                                    <ul className="mb-6 space-y-2 font-mono text-sm text-white/50">
                                        <li className="flex items-center gap-2">
                                            <span className="size-1.5 rounded-full bg-red-400" />
                                            Stop all runners associated with this installation
                                        </li>
                                        <li className="flex items-center gap-2">
                                            <span className="size-1.5 rounded-full bg-red-400" />
                                            Remove all machine configurations
                                        </li>
                                        <li className="flex items-center gap-2">
                                            <span className="size-1.5 rounded-full bg-red-400" />
                                            Revoke access to all repositories
                                        </li>
                                        <li className="flex items-center gap-2">
                                            <span className="size-1.5 rounded-full bg-red-400" />
                                            Delete all associated data
                                        </li>
                                    </ul>
                                    <button
                                        onClick={handleEditInstallation}
                                        className="inline-flex items-center gap-2 rounded-lg border border-red-500/50 bg-red-500/20 px-4 py-2.5 font-mono text-sm text-red-400 transition-colors hover:bg-red-500/30"
                                    >
                                        <RiShieldLine className="size-4" />
                                        Uninstall on GitHub
                                        <RiExternalLinkLine className="size-4" />
                                    </button>
                                    <p className="mt-3 font-mono text-xs text-white/40">
                                        This action is performed through GitHub and cannot be undone.
                                    </p>
                                </div>
                            </motion.div>
                        </motion.div>
                    )}
                </div>
            </div>
        </main>
    );
}
