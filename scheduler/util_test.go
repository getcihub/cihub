package scheduler

import (
	"testing"

	"github.com/getcihub/cihub/core"
)

func TestMachineMatches(t *testing.T) {
	tests := []struct {
		name    string
		machine *core.Machine
		runner  *core.Runner
		want    bool
	}{
		{
			name: "machine offline should reject",
			machine: &core.Machine{
				Name:         "offline-machine",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOffline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "paused machine should reject",
			machine: &core.Machine{
				Name:         "paused-machine",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusPaused,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "owner mismatch should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "bob",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "architecture mismatch amd64 vs arm64 should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchArm64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "architecture mismatch arm64 vs amd64 should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchArm64,
				CPU:          8,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "CPU allocation exceeds explicit limit should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     4,
				CPUAllocated: 3,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "CPU allocation exactly at limit should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     4,
				CPUAllocated: 2,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
		{
			name: "CPU allocation exceeds total CPU (no explicit limit) should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          4,
				CPULimit:     0, // No limit, use CPU
				CPUAllocated: 3,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "CPU allocation within total CPU (no explicit limit) should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0, // No limit, use CPU
				CPUAllocated: 3,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
		{
			name: "RAM allocation exceeds explicit limit should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				RAMLimit:     4096,
				RAMAllocated: 3072,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "RAM allocation exactly at limit should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				RAMLimit:     4096,
				RAMAllocated: 2048,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
		{
			name: "RAM allocation exceeds available RAM (no explicit limit) should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 2048,
				RAMLimit:     0, // No limit, use RAMAvailable
				RAMAllocated: 1024,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "RAM allocation within available RAM (no explicit limit) should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				RAMLimit:     0, // No limit, use RAMAvailable
				RAMAllocated: 4096,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
		{
			name: "runner RAM exceeds available RAM on machine should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 1024,
				RAMLimit:     8192,
				RAMAllocated: 0,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "all checks pass with explicit limits should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     4,
				CPUAllocated: 1,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				RAMLimit:     6144,
				RAMAllocated: 1024,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
		{
			name: "all checks pass without explicit limits should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     0,
				CPUAllocated: 3,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				RAMLimit:     0,
				RAMAllocated: 4096,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
		{
			name: "zero CPU and RAM requirements should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchAmd64,
				CPU:          8,
				CPULimit:     4,
				CPUAllocated: 0,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				RAMLimit:     6144,
				RAMAllocated: 0,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   0,
				RAM:   0,
			},
			want: true,
		},
		{
			name: "architecture unknown should reject",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchUnknown,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchAmd64,
				CPU:   2,
				RAM:   2048,
			},
			want: false,
		},
		{
			name: "both unknown architecture should accept",
			machine: &core.Machine{
				Name:         "machine1",
				Owner:        "alice",
				Arch:         core.ArchUnknown,
				CPU:          8,
				CPULimit:     0,
				RAMTotal:     16384,
				RAMAvailable: 8192,
				Status:       core.MachineStatusOnline,
			},
			runner: &core.Runner{
				Owner: "alice",
				Arch:  core.ArchUnknown,
				CPU:   2,
				RAM:   2048,
			},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := machineMatches(tt.machine, tt.runner)
			if got != tt.want {
				t.Errorf("machineMatches %v, want %v", got, tt.want)
			}
		})
	}
}
