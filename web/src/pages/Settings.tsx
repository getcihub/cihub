import { motion } from 'framer-motion';
import { AppHeader } from '@/components/AppHeader';
import { PageContainer } from '@/components/PageContainer';
import { Card } from '@/components/Card';
import { RiAccountCircleLine, RiShieldLine, RiToggleLine } from '@remixicon/react';

export default function SettingsPage() {
  return (
    <div className="min-h-screen bg-[#050507] text-white grid-bg">
      <AppHeader />

      <PageContainer className="py-8">
        <div className="mb-8">
          <p className="font-mono text-[10px] uppercase tracking-[0.2em] text-white/40">
            Account
          </p>
          <h1 className="mt-2 font-display text-3xl text-white">Settings</h1>
          <p className="mt-2 font-mono text-xs text-muted">
            Manage your profile and preferences.
          </p>
        </div>

        <div className="space-y-6">
          <motion.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.4 }}
            className="grid gap-4 md:grid-cols-2"
          >
            <Card padding="md" className="relative overflow-hidden">
              <div className="absolute left-0 top-0 h-full w-1 bg-cyan-500/70" />
              <div className="flex items-start justify-between">
                <div>
                  <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                    Profile
                  </div>
                  <div className="font-mono text-xs text-white/60">
                    Profile settings are managed via your GitHub account.
                  </div>
                </div>
                <div className="rounded-lg bg-cyan-500/10 p-2">
                  <RiAccountCircleLine className="size-5 text-cyan-400" />
                </div>
              </div>
              <div className="mt-4 space-y-3">
                <div>
                  <label className="mb-1 block font-mono text-[10px] uppercase tracking-wider text-white/40">
                    Display Name
                  </label>
                  <input
                    type="text"
                    value="GitHub User"
                    disabled
                    className="w-full rounded-md border border-white/10 bg-[#0a0a0c] px-3 py-2 font-mono text-xs text-white/40"
                  />
                </div>
                <div>
                  <label className="mb-1 block font-mono text-[10px] uppercase tracking-wider text-white/40">
                    Email
                  </label>
                  <input
                    type="text"
                    value="user@github.com"
                    disabled
                    className="w-full rounded-md border border-white/10 bg-[#0a0a0c] px-3 py-2 font-mono text-xs text-white/40"
                  />
                </div>
              </div>
            </Card>

            <Card padding="md" className="relative overflow-hidden">
              <div className="absolute left-0 top-0 h-full w-1 bg-amber-500/70" />
              <div className="flex items-start justify-between">
                <div>
                  <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                    Preferences
                  </div>
                  <div className="font-mono text-xs text-white/60">
                    Control your default view and update cadence.
                  </div>
                </div>
                <div className="rounded-lg bg-amber-500/10 p-2">
                  <RiToggleLine className="size-5 text-amber-400" />
                </div>
              </div>
              <div className="mt-4 space-y-3">
                <div className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2">
                  <div>
                    <p className="font-mono text-xs text-white">Auto-refresh</p>
                    <p className="font-mono text-[10px] text-white/40">Every 5 seconds</p>
                  </div>
                  <span className="rounded-full bg-emerald-500/20 px-2.5 py-1 font-mono text-[10px] text-emerald-400">
                    Enabled
                  </span>
                </div>
                <div className="flex items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2">
                  <div>
                    <p className="font-mono text-xs text-white">Default tab</p>
                    <p className="font-mono text-[10px] text-white/40">Machines</p>
                  </div>
                  <span className="rounded-full bg-white/5 px-2.5 py-1 font-mono text-[10px] text-white/40">
                    Locked
                  </span>
                </div>
              </div>
            </Card>
          </motion.div>

          <motion.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.4, delay: 0.1 }}
          >
            <Card padding="md" className="relative overflow-hidden">
              <div className="absolute left-0 top-0 h-full w-1 bg-purple-500/70" />
              <div className="flex items-start justify-between">
                <div>
                  <div className="text-[10px] font-mono uppercase tracking-[0.2em] text-white/40 mb-2">
                    Security
                  </div>
                  <div className="font-mono text-xs text-white/60">
                    End your current session and return to sign-in.
                  </div>
                </div>
                <div className="rounded-lg bg-purple-500/10 p-2">
                  <RiShieldLine className="size-5 text-purple-400" />
                </div>
              </div>
              <div className="mt-4">
                <button
                  onClick={() => { window.location.href = '/logout'; }}
                  className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 font-mono text-xs text-red-400 transition-colors hover:bg-red-500/20"
                >
                  Sign Out
                </button>
              </div>
            </Card>
          </motion.div>
        </div>
      </PageContainer>
    </div>
  );
}
