package scheduler

import "github.com/getcihub/cihub/core"

// machineMatches tests if the machine matches the runner requirements.
func machineMatches(m *core.Machine, r *core.Runner) bool {
	// check if machine is online
	if m.Status != core.MachineStatusOnline {
		return false
	}

	// check if owner matches
	if m.Owner != r.Owner {
		return false
	}

	// check if architecture matches
	if r.Arch != m.Arch {
		return false
	}

	// determine effective CPU limit (use CPU total if CPULimit is 0)
	cpuLimit := m.CPULimit
	if cpuLimit == 0 {
		cpuLimit = m.CPU
	}

	// check if CPU allocation won't exceed limit
	if m.CPUAllocated+r.CPU > cpuLimit {
		return false
	}

	// determine effective RAM limit (use RAMAvailable if RAMLimit is 0)
	ramLimit := m.RAMLimit
	if ramLimit == 0 {
		ramLimit = m.RAMAvailable
	}

	// check if RAM allocation won't exceed limit
	if m.RAMAllocated+r.RAM > ramLimit {
		return false
	}

	// check if RAM available on machine is sufficient
	if r.RAM > m.RAMAvailable {
		return false
	}

	return true
}
