import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { RiCloseLine, RiCheckLine, RiFileCopyLine, RiAlertLine } from '@remixicon/react';
import { Button } from './Button';
import type { MachineWithToken, MachineInput } from '@/types/api';

interface CreateMachineDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  owner: string;
  onSuccess: () => void;
}

export function CreateMachineDialog({
  open,
  onOpenChange,
  owner,
  onSuccess,
}: CreateMachineDialogProps) {
  const [formData, setFormData] = useState<MachineInput>({
    name: '',
    arch: 'amd64',
    labels: [],
  });
  const [labelsInput, setLabelsInput] = useState('');
  const [cpuLimit, setCpuLimit] = useState('');
  const [ramLimit, setRamLimit] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const validateName = (name: string): string | null => {
    if (!name) return 'Name is required';
    if (name.length < 3) return 'Name must be at least 3 characters';
    if (name.length > 63) return 'Name must not exceed 63 characters';
    if (!/^[a-z0-9-]+$/.test(name)) {
      return 'Name must contain only lowercase letters, numbers, and hyphens';
    }
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const nameError = validateName(formData.name);
    if (nameError) {
      setError(nameError);
      return;
    }

    setIsSubmitting(true);

    try {
      const payload: MachineInput = {
        name: formData.name,
        arch: formData.arch,
      };

      if (cpuLimit || ramLimit) {
        payload.limit = {
          cpu: cpuLimit ? parseInt(cpuLimit, 10) : 0,
          ram: ramLimit ? parseInt(ramLimit, 10) * 1024 * 1024 * 1024 : 0,
        };
      }

      if (labelsInput.trim()) {
        payload.labels = labelsInput
          .split(',')
          .map((l) => l.trim())
          .filter((l) => l && !l.startsWith('cihub-'))
          .slice(0, 10);
      }

      const res = await fetch(`/api/installations/${owner}/machines`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!res.ok) {
        const data = await res.json();
        throw new Error(data.reason || 'Failed to create machine');
      }

      const data = await res.json();
      const machine: MachineWithToken = data.data;
      setToken(machine.token);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create machine');
      setIsSubmitting(false);
    }
  };

  const handleCopyToken = async () => {
    if (token) {
      await navigator.clipboard.writeText(token);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleClose = () => {
    if (token) {
      onSuccess();
    }
    onOpenChange(false);
    setTimeout(() => {
      setFormData({ name: '', arch: 'amd64', labels: [] });
      setLabelsInput('');
      setCpuLimit('');
      setRamLimit('');
      setError(null);
      setToken(null);
      setCopied(false);
      setIsSubmitting(false);
    }, 200);
  };

  return (
    <Dialog.Root open={open} onOpenChange={handleClose}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm animate-fade-in" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-white/10 bg-[#050507] p-6 shadow-2xl animate-fade-in">
          {token ? (
            <>
              <Dialog.Title className="mb-4 font-display text-xl text-white">
                Machine Created Successfully
              </Dialog.Title>
              <div className="space-y-4">
                <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/5 p-4">
                  <div className="flex items-start gap-3">
                    <RiCheckLine className="mt-0.5 size-5 flex-shrink-0 text-emerald-400" />
                    <div className="flex-1">
                      <p className="font-mono text-sm text-white">
                        Your machine has been created successfully.
                      </p>
                      <p className="mt-1 font-mono text-xs text-emerald-400/70">
                        Copy the token below - it won't be shown again.
                      </p>
                    </div>
                  </div>
                </div>

                <div>
                  <label className="mb-2 block font-mono text-xs uppercase tracking-wider text-secondary">
                    Machine Token
                  </label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={token}
                      readOnly
                      className="flex-1 rounded-md border border-white/10 bg-white/5 px-3 py-2 font-mono text-xs text-white focus:border-amber-500/50 focus:outline-none focus:ring-2 focus:ring-amber-500/20"
                    />
                    <Button
                      size="md"
                      variant={copied ? 'secondary' : 'primary'}
                      onClick={handleCopyToken}
                    >
                      {copied ? (
                        <>
                          <RiCheckLine className="size-4" />
                          Copied
                        </>
                      ) : (
                        <>
                          <RiFileCopyLine className="size-4" />
                          Copy
                        </>
                      )}
                    </Button>
                  </div>
                </div>

                <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-4">
                  <div className="flex items-start gap-3">
                    <RiAlertLine className="mt-0.5 size-5 flex-shrink-0 text-amber-400" />
                    <p className="font-mono text-xs text-amber-400/80">
                      Save this token securely. You'll need it to authenticate the machine.
                      This token will not be shown again.
                    </p>
                  </div>
                </div>

                <Button variant="primary" className="w-full" onClick={handleClose}>
                  Done
                </Button>
              </div>
            </>
          ) : (
            <>
              <Dialog.Title className="mb-4 font-display text-xl text-white">
                Create New Machine
              </Dialog.Title>
              <Dialog.Description className="mb-6 font-mono text-sm text-muted">
                Create a new machine to host self-hosted runners for {owner}.
              </Dialog.Description>

              <form onSubmit={handleSubmit} className="space-y-4">
                {error && (
                  <div className="rounded-lg border border-red-500/20 bg-red-500/5 p-3">
                    <p className="font-mono text-xs text-red-400">{error}</p>
                  </div>
                )}

                <div>
                  <label
                    htmlFor="name"
                    className="mb-2 block font-mono text-xs uppercase tracking-wider text-secondary"
                  >
                    Machine Name *
                  </label>
                  <input
                    id="name"
                    type="text"
                    value={formData.name}
                    onChange={(e) =>
                      setFormData({ ...formData, name: e.target.value })
                    }
                    placeholder="my-runner-1"
                    className="w-full rounded-md border border-white/10 bg-white/5 px-3 py-2 font-mono text-sm text-white placeholder:text-white/30 focus:border-amber-500/50 focus:outline-none focus:ring-2 focus:ring-amber-500/20"
                    required
                  />
                  <p className="mt-1 font-mono text-xs text-muted">
                    Lowercase letters, numbers, and hyphens only
                  </p>
                </div>

                <div>
                  <label
                    htmlFor="arch"
                    className="mb-2 block font-mono text-xs uppercase tracking-wider text-secondary"
                  >
                    Architecture *
                  </label>
                  <select
                    id="arch"
                    value={formData.arch}
                    onChange={(e) =>
                      setFormData({ ...formData, arch: e.target.value })
                    }
                    className="w-full rounded-md border border-white/10 bg-white/5 px-3 py-2 font-mono text-sm text-white focus:border-amber-500/50 focus:outline-none focus:ring-2 focus:ring-amber-500/20"
                    required
                  >
                    <option value="amd64">amd64 (x86_64)</option>
                    <option value="arm64">arm64 (aarch64)</option>
                  </select>
                </div>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label
                      htmlFor="cpu_limit"
                      className="mb-2 block font-mono text-xs uppercase tracking-wider text-secondary"
                    >
                      CPU Limit
                    </label>
                    <input
                      id="cpu_limit"
                      type="number"
                      min="0"
                      value={cpuLimit}
                      onChange={(e) => setCpuLimit(e.target.value)}
                      placeholder="0 = unlimited"
                      className="w-full rounded-md border border-white/10 bg-white/5 px-3 py-2 font-mono text-sm text-white placeholder:text-white/30 focus:border-amber-500/50 focus:outline-none focus:ring-2 focus:ring-amber-500/20"
                    />
                  </div>

                  <div>
                    <label
                      htmlFor="ram_limit"
                      className="mb-2 block font-mono text-xs uppercase tracking-wider text-secondary"
                    >
                      RAM Limit (GB)
                    </label>
                    <input
                      id="ram_limit"
                      type="number"
                      min="0"
                      value={ramLimit}
                      onChange={(e) => setRamLimit(e.target.value)}
                      placeholder="0 = unlimited"
                      className="w-full rounded-md border border-white/10 bg-white/5 px-3 py-2 font-mono text-sm text-white placeholder:text-white/30 focus:border-amber-500/50 focus:outline-none focus:ring-2 focus:ring-amber-500/20"
                    />
                  </div>
                </div>

                <div>
                  <label
                    htmlFor="labels"
                    className="mb-2 block font-mono text-xs uppercase tracking-wider text-secondary"
                  >
                    Labels (Optional)
                  </label>
                  <input
                    id="labels"
                    type="text"
                    value={labelsInput}
                    onChange={(e) => setLabelsInput(e.target.value)}
                    placeholder="linux, x64, gpu"
                    className="w-full rounded-md border border-white/10 bg-white/5 px-3 py-2 font-mono text-sm text-white placeholder:text-white/30 focus:border-amber-500/50 focus:outline-none focus:ring-2 focus:ring-amber-500/20"
                  />
                  <p className="mt-1 font-mono text-xs text-muted">
                    Comma-separated. Max 10 labels.
                  </p>
                </div>

                <div className="flex gap-3 pt-2">
                  <Button
                    type="button"
                    variant="secondary"
                    className="flex-1"
                    onClick={handleClose}
                    disabled={isSubmitting}
                  >
                    Cancel
                  </Button>
                  <Button
                    type="submit"
                    variant="primary"
                    className="flex-1"
                    disabled={isSubmitting}
                  >
                    {isSubmitting ? 'Creating...' : 'Create Machine'}
                  </Button>
                </div>
              </form>
            </>
          )}

          <Dialog.Close asChild>
            <button
              className="absolute right-4 top-4 rounded-md p-1 text-white/40 hover:bg-white/10 hover:text-white/70"
              aria-label="Close"
            >
              <RiCloseLine className="size-5" />
            </button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
