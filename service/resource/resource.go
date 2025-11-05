package resource

import (
	"context"
	"fmt"

	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/mem"

	"github.com/getcihub/cihub/core"
)

type service struct{}

// New creates a new ResourceService that reports machine resources using gopsutil.
func New() core.ResourceService {
	return &service{}
}

// Report reports the current machine resources including CPU count, RAM, and architecture.
func (s *service) Report(ctx context.Context) (*core.Resource, error) {
	// Get CPU count (logical CPUs/threads).
	cpuCount, err := cpu.CountsWithContext(ctx, true)
	if err != nil {
		return nil, fmt.Errorf("resource: failed to get CPU count: %w", err)
	}

	// Get memory information
	memInfo, err := mem.VirtualMemoryWithContext(ctx)
	if err != nil {
		return nil, fmt.Errorf("resource: failed to get memory info: %w", err)
	}

	return &core.Resource{
		Arch:         detectCPUArch(),
		CPU:          int64(cpuCount),
		RAMTotal:     int64(memInfo.Total / (1024 * 1024)),
		RAMAvailable: int64(memInfo.Available / (1024 * 1024)),
	}, nil
}
