import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from '@/components/Dialog';
import { useMachineMutations } from '@/hooks/useMachineMutations';
import {
    RiAddLine,
    RiCheckLine,
    RiCloseLine,
    RiFileCopyLine,
} from '@remixicon/react';
import { useState } from 'react';
import { toast } from 'sonner';

interface AddMachineModalProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
}

export function AddMachineModal({ open, onOpenChange }: AddMachineModalProps) {
    const { createMachine } = useMachineMutations();

    const [step, setStep] = useState<'form' | 'instructions'>('form');
    const [autoDetect, setAutoDetect] = useState(true);
    const [formData, setFormData] = useState({
        name: '',
        cpu: '',
        ram: '',
    });
    const [labels, setLabels] = useState<string[]>([]);
    const [newLabel, setNewLabel] = useState('');
    const [machineToken, setMachineToken] = useState<string | null>(null);
    const [copiedToken, setCopiedToken] = useState(false);
    const [copiedCommand, setCopiedCommand] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const resetForm = () => {
        setStep('form');
        setAutoDetect(true);
        setFormData({ name: '', cpu: '', ram: '' });
        setLabels([]);
        setNewLabel('');
        setMachineToken(null);
        setCopiedToken(false);
        setCopiedCommand(false);
        setError(null);
    };

    const handleClose = () => {
        resetForm();
        onOpenChange(false);
    };

    const handleInputChange = (
        e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>,
    ) => {
        const { name, value } = e.target;
        setFormData((prev) => ({
            ...prev,
            [name]: value,
        }));
    };

    const handleAddLabel = () => {
        const newLabels = newLabel
            .split(',')
            .map((label) => label.trim())
            .filter((label) => label && !labels.includes(label));

        if (newLabels.length > 0) {
            setLabels([...labels, ...newLabels]);
            setNewLabel('');
        }
    };

    const handleRemoveLabel = (labelToRemove: string) => {
        setLabels(labels.filter((label) => label !== labelToRemove));
    };

    const handleLabelKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            handleAddLabel();
        }
    };

    const handleFormSubmit = async (e: React.FormEvent) => {
        e.preventDefault();

        if (!formData.name.trim()) {
            setError('Machine name is required');
            toast.error('Machine name is required');
            return;
        }

        if (formData.cpu && parseInt(formData.cpu) < 1) {
            setError('CPU must be at least 1');
            toast.error('CPU must be at least 1');
            return;
        }

        if (formData.ram && parseInt(formData.ram) < 512) {
            setError('RAM must be at least 512 MB');
            toast.error('RAM must be at least 512 MB');
            return;
        }

        setError(null);

        const createMachinePromise = async () => {
            const payload: {
                name: string;
                labels: string[];
                cpu?: number;
                ram?: number;
            } = {
                name: formData.name,
                labels,
            };

            if (formData.cpu) {
                payload.cpu = parseInt(formData.cpu);
            }

            if (formData.ram) {
                payload.ram = parseInt(formData.ram);
            }

            const response = await createMachine.mutateAsync(payload);

            if (response.data?.token) {
                setMachineToken(response.data.token);
                setStep('instructions');
                return { success: true, name: formData.name };
            } else {
                throw new Error('Failed to get machine token from server');
            }
        };

        toast.promise(createMachinePromise(), {
            loading: `Creating machine "${formData.name}"...`,
            success: (data) => `Machine "${data.name}" created successfully`,
            error: (err) =>
                `Failed to create machine: ${err instanceof Error ? err.message : 'Unknown error'}`,
        });
    };

    const cihubServer =
        typeof window !== 'undefined'
            ? window.location.origin
            : 'https://cloud.cihub.io';
    const installCommand = `
# Download and run the agent installer
curl -LsSf "https://install.cihub.io" | sudo bash -s -- \\
  --token "${machineToken}" \\
  --server "${cihubServer}"`;

    const handleCopyCommand = () => {
        navigator.clipboard.writeText(installCommand);
        setCopiedCommand(true);
        toast.success('Installation command copied to clipboard');
        setTimeout(() => setCopiedCommand(false), 2000);
    };

    const handleCopyToken = () => {
        if (machineToken) {
            navigator.clipboard.writeText(machineToken);
            setCopiedToken(true);
            toast.success('Machine token copied to clipboard');
            setTimeout(() => setCopiedToken(false), 2000);
        }
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-2xl">
                {step === 'form' ? (
                    <>
                        <DialogHeader>
                            <DialogTitle>Add Machine</DialogTitle>
                            <DialogDescription>
                                Create a new machine and get the installation
                                instructions
                            </DialogDescription>
                        </DialogHeader>

                        {error && (
                            <div className="mb-4 rounded-lg border border-red-500/20 bg-red-500/10 p-4">
                                <div className="flex gap-3">
                                    <RiCloseLine
                                        className="mt-0.5 size-4 flex-shrink-0 text-red-400"
                                        aria-hidden="true"
                                    />
                                    <p className="font-mono text-xs text-red-400">
                                        {error}
                                    </p>
                                </div>
                            </div>
                        )}

                        <form onSubmit={handleFormSubmit} className="space-y-5">
                            {/* Machine Name */}
                            <div>
                                <label
                                    htmlFor="name"
                                    className="mb-2 block font-mono text-xs text-white/60"
                                >
                                    Machine Name{' '}
                                    <span className="text-red-400">*</span>
                                </label>
                                <input
                                    type="text"
                                    id="name"
                                    name="name"
                                    value={formData.name}
                                    onChange={handleInputChange}
                                    placeholder="worker-01"
                                    className="w-full rounded-md border border-white/10 bg-white/[0.02] px-3 py-2 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                                    required
                                />
                                <p className="mt-1 font-mono text-[11px] text-white/30">
                                    A unique identifier for this machine
                                </p>
                            </div>

                            {/* Labels */}
                            <div>
                                <label
                                    htmlFor="label"
                                    className="mb-2 block font-mono text-xs text-white/60"
                                >
                                    Labels{' '}
                                    <span className="font-normal text-white/30">
                                        (optional)
                                    </span>
                                </label>
                                <div className="mb-2 flex gap-2">
                                    <input
                                        type="text"
                                        id="label"
                                        value={newLabel}
                                        onChange={(e) =>
                                            setNewLabel(e.target.value)
                                        }
                                        onKeyDown={handleLabelKeyDown}
                                        placeholder="gpu, production"
                                        className="flex-1 rounded-md border border-white/10 bg-white/[0.02] px-3 py-2 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                                    />
                                    <button
                                        type="button"
                                        onClick={handleAddLabel}
                                        className="flex items-center gap-1.5 rounded-md bg-amber-500 px-3 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400"
                                    >
                                        <RiAddLine className="size-4" />
                                        Add
                                    </button>
                                </div>
                                {labels.length > 0 && (
                                    <div className="flex flex-wrap gap-2">
                                        {labels.map((label) => (
                                            <span
                                                key={label}
                                                className="inline-flex items-center gap-1.5 rounded-full bg-white/5 px-2.5 py-1 font-mono text-xs text-white/70"
                                            >
                                                {label}
                                                <button
                                                    type="button"
                                                    onClick={() =>
                                                        handleRemoveLabel(label)
                                                    }
                                                    className="text-white/40 transition-colors hover:text-white/70"
                                                >
                                                    <RiCloseLine className="size-3.5" />
                                                </button>
                                            </span>
                                        ))}
                                    </div>
                                )}
                            </div>

                            {/* Auto-detect toggle */}
                            <div className="flex items-center justify-between gap-4 border-t border-white/10 pt-5">
                                <div className="flex-1">
                                    <label
                                        htmlFor="autoDetect"
                                        className="font-mono text-xs text-white/60"
                                    >
                                        Let the agent detect specs
                                    </label>
                                    <p className="mt-1 font-mono text-[11px] text-white/30">
                                        CPU and RAM will be detected
                                        automatically
                                    </p>
                                </div>
                                <button
                                    type="button"
                                    id="autoDetect"
                                    onClick={() => setAutoDetect(!autoDetect)}
                                    className={`relative inline-flex h-6 w-11 flex-shrink-0 items-center rounded-full transition-colors ${
                                        autoDetect
                                            ? 'bg-amber-500'
                                            : 'bg-white/20'
                                    }`}
                                >
                                    <span
                                        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                                            autoDetect
                                                ? 'translate-x-6'
                                                : 'translate-x-1'
                                        }`}
                                    />
                                </button>
                            </div>

                            {!autoDetect && (
                                <div className="grid grid-cols-2 gap-4">
                                    {/* CPU */}
                                    <div>
                                        <label
                                            htmlFor="cpu"
                                            className="mb-2 block font-mono text-xs text-white/60"
                                        >
                                            CPU Cores
                                        </label>
                                        <input
                                            type="number"
                                            id="cpu"
                                            name="cpu"
                                            value={formData.cpu}
                                            onChange={handleInputChange}
                                            min="1"
                                            placeholder="4"
                                            className="w-full rounded-md border border-white/10 bg-white/[0.02] px-3 py-2 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                                        />
                                    </div>

                                    {/* RAM */}
                                    <div>
                                        <label
                                            htmlFor="ram"
                                            className="mb-2 block font-mono text-xs text-white/60"
                                        >
                                            RAM (MB)
                                        </label>
                                        <input
                                            type="number"
                                            id="ram"
                                            name="ram"
                                            value={formData.ram}
                                            onChange={handleInputChange}
                                            min="512"
                                            step="512"
                                            placeholder="8192"
                                            className="w-full rounded-md border border-white/10 bg-white/[0.02] px-3 py-2 font-mono text-sm text-white placeholder-white/30 outline-none transition-colors focus:border-white/30"
                                        />
                                    </div>
                                </div>
                            )}

                            {/* Buttons */}
                            <div className="flex justify-end gap-3 pt-2">
                                <button
                                    type="button"
                                    onClick={handleClose}
                                    className="inline-flex items-center rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                                >
                                    Cancel
                                </button>
                                <button
                                    type="submit"
                                    disabled={createMachine.isPending}
                                    className="inline-flex items-center gap-2 rounded-md bg-amber-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400 disabled:cursor-not-allowed disabled:opacity-50"
                                >
                                    {createMachine.isPending
                                        ? 'Creating...'
                                        : 'Create Machine'}
                                </button>
                            </div>
                        </form>
                    </>
                ) : (
                    <>
                        <DialogHeader>
                            <DialogTitle>Installation Instructions</DialogTitle>
                            <DialogDescription>
                                Machine "{formData.name}" is ready for setup
                            </DialogDescription>
                        </DialogHeader>

                        {/* Success Message */}
                        <div className="mb-4 rounded-lg border border-emerald-500/20 bg-emerald-500/10 p-4">
                            <div className="flex gap-3">
                                <RiCheckLine
                                    className="mt-0.5 size-4 flex-shrink-0 text-emerald-400"
                                    aria-hidden="true"
                                />
                                <div>
                                    <p className="font-mono text-xs text-emerald-400">
                                        Machine created! Copy the command below
                                        and run it on your machine.
                                    </p>
                                </div>
                            </div>
                        </div>

                        {/* Machine Token */}
                        <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/10 p-4">
                            <p className="mb-2 font-mono text-xs font-medium text-amber-400">
                                Machine Token
                            </p>
                            <div className="flex gap-2">
                                <div className="flex-1 break-all rounded border border-amber-500/30 bg-[#050507] p-2 font-mono text-xs text-white">
                                    {machineToken}
                                </div>
                                <button
                                    onClick={handleCopyToken}
                                    className="flex flex-shrink-0 items-center gap-1.5 rounded bg-amber-500/20 px-3 py-1.5 font-mono text-xs text-amber-400 transition-colors hover:bg-amber-500/30"
                                >
                                    <RiFileCopyLine className="size-3.5" />
                                    {copiedToken ? 'Copied!' : 'Copy'}
                                </button>
                            </div>
                        </div>

                        {/* Installation Command */}
                        <div className="mb-4">
                            <p className="mb-2 font-mono text-xs text-white/60">
                                Installation Command
                            </p>
                            <div className="mb-2 overflow-x-auto rounded-lg bg-[#050507] p-3 font-mono text-xs text-white/90">
                                <pre className="whitespace-pre-wrap">
                                    {installCommand}
                                </pre>
                            </div>
                            <button
                                onClick={handleCopyCommand}
                                className="inline-flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] px-3 py-1.5 font-mono text-xs text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                            >
                                <RiFileCopyLine className="size-3.5" />
                                {copiedCommand ? 'Copied!' : 'Copy Command'}
                            </button>
                        </div>

                        {/* Action Buttons */}
                        <div className="flex justify-end gap-3 pt-2">
                            <button
                                onClick={() => {
                                    resetForm();
                                }}
                                className="inline-flex items-center rounded-md border border-white/10 bg-white/[0.02] px-4 py-2 font-mono text-sm text-white/70 transition-all hover:border-white/20 hover:bg-white/5 hover:text-white"
                            >
                                Add Another
                            </button>
                            <button
                                onClick={handleClose}
                                className="inline-flex items-center rounded-md bg-amber-500 px-4 py-2 font-mono text-sm text-white transition-colors hover:bg-amber-400"
                            >
                                Done
                            </button>
                        </div>
                    </>
                )}
            </DialogContent>
        </Dialog>
    );
}
