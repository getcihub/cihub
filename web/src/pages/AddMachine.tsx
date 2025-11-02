import { useState } from 'react'
import { useNavigate, useParams } from '@tanstack/react-router'
import { RiArrowLeftLine, RiCheckLine, RiFileCopyLine, RiAddLine, RiCloseLine } from '@remixicon/react'
import { useInstallation } from '../hooks/useInstallation'
import { Card } from '../components/Card'
import { Button } from '../components/Button'

export function AddMachinePage() {
    const navigate = useNavigate()
    const { login } = useParams({ from: '/$login/machines/add' })
    const { selectedInstallation } = useInstallation()

    const [step, setStep] = useState<'form' | 'instructions'>('form')
    const [autoDetect, setAutoDetect] = useState(true)
    const [formData, setFormData] = useState({
        name: '',
        arch: 'x86_64',
        cpu: '4',
        ram: '8192',
    })
    const [labels, setLabels] = useState<string[]>([])
    const [newLabel, setNewLabel] = useState('')
    const [machineToken, setMachineToken] = useState<string | null>(null)
    const [copied, setCopied] = useState(false)

    const handleBack = () => {
        navigate({ to: '/$login/machines', params: { login } })
    }

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
        const { name, value } = e.target
        setFormData((prev) => ({
            ...prev,
            [name]: value,
        }))
    }

    const handleAddLabel = () => {
        if (newLabel.trim() && !labels.includes(newLabel.trim())) {
            setLabels([...labels, newLabel.trim()])
            setNewLabel('')
        }
    }

    const handleRemoveLabel = (labelToRemove: string) => {
        setLabels(labels.filter((label) => label !== labelToRemove))
    }

    const handleLabelKeyPress = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
            e.preventDefault()
            handleAddLabel()
        }
    }

    const handleFormSubmit = (e: React.FormEvent) => {
        e.preventDefault()

        // Validate form
        if (!formData.name.trim()) {
            alert('Machine name is required')
            return
        }

        if (parseInt(formData.cpu) < 1) {
            alert('CPU must be at least 1')
            return
        }

        if (parseInt(formData.ram) < 512) {
            alert('RAM must be at least 512 MB')
            return
        }

        // Generate mock token (in real app, this would come from the server)
        const token = `cihub_${Date.now()}_${Math.random().toString(36).substring(2, 15)}`
        setMachineToken(token)
        setStep('instructions')
    }

    const installCommand = `#!/bin/bash
# CIHub Agent Installation Script
# Machine: ${formData.name}
# Installation: ${selectedInstallation?.login}

MACHINE_TOKEN="${machineToken}"
MACHINE_NAME="${formData.name}"
INSTALLATION="${selectedInstallation?.login}"
CIHUB_SERVER="https://cihub.example.com"

# Download and run the agent installer
curl -sSL "$CIHUB_SERVER/agent/install.sh" | bash -s -- \\
  --machine-token "$MACHINE_TOKEN" \\
  --machine-name "$MACHINE_NAME" \\
  --installation "$INSTALLATION" \\
  --server "$CIHUB_SERVER"`

    const handleCopyCommand = () => {
        navigator.clipboard.writeText(installCommand)
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
    }

    return (
        <div className="space-y-6 max-w-2xl">
            {/* Back Button */}
            <button
                onClick={handleBack}
                className="text-blue-600 hover:text-blue-700 flex items-center gap-2 font-medium"
            >
                <RiArrowLeftLine className="size-4" aria-hidden="true" />
                Back to Machines
            </button>

            {step === 'form' ? (
                <>
                    <div>
                        <h1 className="text-3xl font-bold text-gray-900">Add Machine</h1>
                        <p className="text-gray-600 mt-2">Create a new machine and get the installation instructions</p>
                    </div>

                    <Card className="p-8">
                        <form onSubmit={handleFormSubmit} className="space-y-6">
                            {/* Machine Name */}
                            <div>
                                <label htmlFor="name" className="block text-sm font-medium text-gray-700 mb-2">
                                    Machine Name <span className="text-red-500">*</span>
                                </label>
                                <input
                                    type="text"
                                    id="name"
                                    name="name"
                                    value={formData.name}
                                    onChange={handleInputChange}
                                    placeholder="e.g., runner-1, build-server"
                                    className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-xs placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-black focus:border-transparent"
                                    required
                                />
                                <p className="text-sm text-gray-500 mt-1">A unique identifier for this machine</p>
                            </div>

                            {/* Auto Detect Toggle */}
                            <div className="border-t border-gray-200 pt-6">
                                <div className="flex items-center justify-between">
                                    <div>
                                        <label htmlFor="autoDetect" className="text-sm font-medium text-gray-700">
                                            Let the agent detect specs
                                        </label>
                                        <p className="text-sm text-gray-500 mt-1">
                                            Architecture, CPU, and RAM will be detected automatically when the agent connects
                                        </p>
                                    </div>
                                    <button
                                        type="button"
                                        id="autoDetect"
                                        onClick={() => setAutoDetect(!autoDetect)}
                                        className={`relative inline-flex h-6 w-11 items-center rounded-full flex-shrink-0 ${
                                            autoDetect ? 'bg-black' : 'bg-gray-200'
                                        } transition-colors`}
                                    >
                                        <span
                                            className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                                                autoDetect ? 'translate-x-6' : 'translate-x-1'
                                            }`}
                                        />
                                    </button>
                                </div>
                            </div>

                            {/* Labels */}
                            <div>
                                <label htmlFor="label" className="block text-sm font-medium text-gray-700 mb-2">
                                    Labels <span className="text-gray-500 font-normal">(optional)</span>
                                </label>
                                <div className="flex gap-2 mb-3">
                                    <input
                                        type="text"
                                        id="label"
                                        value={newLabel}
                                        onChange={(e) => setNewLabel(e.target.value)}
                                        onKeyPress={handleLabelKeyPress}
                                        placeholder="e.g., gpu, production, testing"
                                        className="flex-1 px-3 py-2 border border-gray-300 rounded-md shadow-xs placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-black focus:border-transparent"
                                    />
                                    <button
                                        type="button"
                                        onClick={handleAddLabel}
                                        className="px-4 py-2 bg-gray-900 text-white rounded-md hover:bg-gray-800 transition-colors flex items-center gap-2"
                                    >
                                        <RiAddLine className="size-4" />
                                        Add
                                    </button>
                                </div>
                                {labels.length > 0 && (
                                    <div className="flex gap-2 flex-wrap">
                                        {labels.map((label) => (
                                            <span
                                                key={label}
                                                className="inline-flex items-center gap-2 px-3 py-1 rounded-full text-sm font-medium bg-gray-100 text-gray-900"
                                            >
                                                {label}
                                                <button
                                                    type="button"
                                                    onClick={() => handleRemoveLabel(label)}
                                                    className="text-gray-500 hover:text-gray-700 transition-colors"
                                                >
                                                    <RiCloseLine className="size-4" />
                                                </button>
                                            </span>
                                        ))}
                                    </div>
                                )}
                                <p className="text-sm text-gray-500 mt-2">Press Enter or click Add to add a label</p>
                            </div>

                            {!autoDetect && (
                                <>
                                    {/* Architecture */}
                                    <div>
                                        <label htmlFor="arch" className="block text-sm font-medium text-gray-700 mb-2">
                                            Architecture <span className="text-red-500">*</span>
                                        </label>
                                        <select
                                            id="arch"
                                            name="arch"
                                            value={formData.arch}
                                            onChange={handleInputChange}
                                            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-xs focus:outline-none focus:ring-1 focus:ring-black focus:border-transparent"
                                        >
                                            <option value="x86_64">x86_64 (Intel/AMD 64-bit)</option>
                                            <option value="arm64">arm64 (ARM 64-bit)</option>
                                            <option value="armv7">armv7 (ARM 32-bit)</option>
                                        </select>
                                    </div>

                                    {/* CPU */}
                                    <div>
                                        <label htmlFor="cpu" className="block text-sm font-medium text-gray-700 mb-2">
                                            CPU Cores <span className="text-red-500">*</span>
                                        </label>
                                        <input
                                            type="number"
                                            id="cpu"
                                            name="cpu"
                                            value={formData.cpu}
                                            onChange={handleInputChange}
                                            min="1"
                                            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-xs focus:outline-none focus:ring-1 focus:ring-black focus:border-transparent"
                                            required
                                        />
                                        <p className="text-sm text-gray-500 mt-1">Number of CPU cores available on this machine</p>
                                    </div>

                                    {/* RAM */}
                                    <div>
                                        <label htmlFor="ram" className="block text-sm font-medium text-gray-700 mb-2">
                                            RAM (MB) <span className="text-red-500">*</span>
                                        </label>
                                        <input
                                            type="number"
                                            id="ram"
                                            name="ram"
                                            value={formData.ram}
                                            onChange={handleInputChange}
                                            min="512"
                                            step="512"
                                            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-xs focus:outline-none focus:ring-1 focus:ring-black focus:border-transparent"
                                            required
                                        />
                                        <p className="text-sm text-gray-500 mt-1">Amount of RAM in megabytes (minimum 512 MB)</p>
                                    </div>
                                </>
                            )}

                            {/* Buttons */}
                            <div className="flex gap-3 pt-4">
                                <Button type="submit" className="gap-2">
                                    Generate Installation Command
                                </Button>
                                <Button type="button" variant="secondary" onClick={handleBack}>
                                    Cancel
                                </Button>
                            </div>
                        </form>
                    </Card>
                </>
            ) : (
                <>
                    <div>
                        <h1 className="text-3xl font-bold text-gray-900">Installation Instructions</h1>
                        <p className="text-gray-600 mt-2">Machine "{formData.name}" is ready for setup</p>
                    </div>

                    {/* Success Message */}
                    <Card className="bg-green-50 border-green-200 p-6">
                        <div className="flex gap-3">
                            <RiCheckLine className="size-5 text-green-600 flex-shrink-0 mt-0.5" aria-hidden="true" />
                            <div>
                                <h3 className="font-medium text-green-900">Machine Created</h3>
                                <p className="text-sm text-green-800 mt-1">
                                    Machine "{formData.name}" has been created. Copy the installation command below and run it on your machine.
                                </p>
                            </div>
                        </div>
                    </Card>

                    {/* Machine Details */}
                    {!autoDetect && (
                        <Card className="p-6">
                            <h2 className="text-lg font-semibold text-gray-900 mb-4">Machine Details</h2>
                            <div className="grid grid-cols-2 gap-6">
                                <div>
                                    <p className="text-sm font-medium text-gray-600">Name</p>
                                    <p className="text-lg text-gray-900 mt-1 font-mono">{formData.name}</p>
                                </div>
                                <div>
                                    <p className="text-sm font-medium text-gray-600">Architecture</p>
                                    <p className="text-lg text-gray-900 mt-1">{formData.arch}</p>
                                </div>
                                <div>
                                    <p className="text-sm font-medium text-gray-600">CPU Cores</p>
                                    <p className="text-lg text-gray-900 mt-1">{formData.cpu}</p>
                                </div>
                                <div>
                                    <p className="text-sm font-medium text-gray-600">RAM</p>
                                    <p className="text-lg text-gray-900 mt-1">{(parseInt(formData.ram) / 1024).toFixed(1)} GB</p>
                                </div>
                            </div>
                        </Card>
                    )}

                    {/* Machine Token */}
                    <Card className="p-6 border-amber-200 bg-amber-50">
                        <h3 className="font-semibold text-amber-900 mb-2">Machine Token</h3>
                        <p className="text-sm text-amber-800 mb-3">
                            Keep this token secure. You will need it to authenticate the agent on this machine.
                        </p>
                        <div className="bg-white border border-amber-300 rounded p-3 font-mono text-sm break-all text-gray-900">
                            {machineToken}
                        </div>
                    </Card>

                    {/* Installation Command */}
                    <Card className="p-6">
                        <h2 className="text-lg font-semibold text-gray-900 mb-4">Installation Command</h2>
                        <p className="text-sm text-gray-600 mb-4">
                            Run the following command on your machine to install the CIHub agent:
                        </p>

                        <div className="bg-gray-900 text-gray-100 rounded-lg p-4 overflow-x-auto font-mono text-sm mb-4">
                            <pre>{installCommand}</pre>
                        </div>

                        <Button
                            onClick={handleCopyCommand}
                            variant="secondary"
                            className="gap-2"
                        >
                            <RiFileCopyLine className="size-4" />
                            {copied ? 'Copied!' : 'Copy Command'}
                        </Button>
                    </Card>

                    {/* Setup Instructions */}
                    <Card className="p-6">
                        <h2 className="text-lg font-semibold text-gray-900 mb-4">Setup Steps</h2>
                        <div className="space-y-4">
                            {!autoDetect && (
                                <div>
                                    <div className="flex gap-3">
                                        <div className="flex-shrink-0 w-6 h-6 rounded-full bg-black text-white text-xs font-bold flex items-center justify-center">
                                            1
                                        </div>
                                        <div>
                                            <h4 className="font-medium text-gray-900">Prepare Your Machine</h4>
                                            <p className="text-sm text-gray-600 mt-1">
                                                Ensure you have bash and curl installed on your machine. The machine should have at least {formData.cpu} CPU cores and {(parseInt(formData.ram) / 1024).toFixed(1)} GB of RAM.
                                            </p>
                                        </div>
                                    </div>
                                </div>
                            )}

                            <div>
                                <div className="flex gap-3">
                                    <div className="flex-shrink-0 w-6 h-6 rounded-full bg-black text-white text-xs font-bold flex items-center justify-center">
                                        {autoDetect ? 1 : 2}
                                    </div>
                                    <div>
                                        <h4 className="font-medium text-gray-900">Copy Installation Command</h4>
                                        <p className="text-sm text-gray-600 mt-1">
                                            Click the "Copy Command" button above to copy the installation script to your clipboard.
                                        </p>
                                    </div>
                                </div>
                            </div>

                            <div>
                                <div className="flex gap-3">
                                    <div className="flex-shrink-0 w-6 h-6 rounded-full bg-black text-white text-xs font-bold flex items-center justify-center">
                                        {autoDetect ? 2 : 3}
                                    </div>
                                    <div>
                                        <h4 className="font-medium text-gray-900">Run the Script</h4>
                                        <p className="text-sm text-gray-600 mt-1">
                                            SSH into your machine and paste the command. The script will download and install the CIHub agent automatically.
                                        </p>
                                    </div>
                                </div>
                            </div>

                            <div>
                                <div className="flex gap-3">
                                    <div className="flex-shrink-0 w-6 h-6 rounded-full bg-black text-white text-xs font-bold flex items-center justify-center">
                                        {autoDetect ? 3 : 4}
                                    </div>
                                    <div>
                                        <h4 className="font-medium text-gray-900">Verify Installation</h4>
                                        <p className="text-sm text-gray-600 mt-1">
                                            Once the script completes, the machine should appear in your machines list with status "online".
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </Card>

                    {/* Action Buttons */}
                    <div className="flex gap-3">
                        <Button onClick={handleBack}>
                            Done
                        </Button>
                        <Button
                            variant="secondary"
                            onClick={() => {
                                setStep('form')
                                setFormData({ name: '', arch: 'x86_64', cpu: '4', ram: '8192' })
                                setMachineToken(null)
                            }}
                        >
                            Add Another Machine
                        </Button>
                    </div>
                </>
            )}
        </div>
    )
}
