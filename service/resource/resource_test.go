package resource

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/getcihub/cihub/core"
)

func abs(n int64) int64 {
	if n < 0 {
		return -n
	}
	return n
}

func TestNew(t *testing.T) {
	svc := New()
	assert.NotNil(t, svc)
	assert.Implements(t, (*core.ResourceService)(nil), svc)
}

func TestReport(t *testing.T) {
	svc := New()
	ctx := context.Background()

	res, err := svc.Report(ctx)
	require.NoError(t, err)
	require.NotNil(t, res)

	// Validate CPU count (should be at least 1)
	assert.Greater(t, res.CPU, int64(0), "CPU count should be greater than 0")

	// Validate RAM in MB (should be at least some reasonable amount, typically >= 512 MB on test machines)
	assert.Greater(t, res.RAMTotal, int64(0), "RAMTotal should be greater than 0")
	assert.Greater(t, res.RAMAvailable, int64(0), "RAMAvailable should be greater than 0")

	// Validate that available RAM is not greater than total RAM
	assert.LessOrEqual(t, res.RAMAvailable, res.RAMTotal, "Available RAM should not exceed total RAM")

	// Validate architecture is recognized
	assert.NotEqual(t, core.ArchUnknown, res.Arch, "Architecture should be detected")
}

func TestReportContextHandling(t *testing.T) {
	svc := New()

	// Test with valid context
	res, err := svc.Report(context.Background())
	require.NoError(t, err)
	assert.NotNil(t, res)

	// Test with context that has timeout (should still work since gopsutil doesn't use context)
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)

	res2, err := svc.Report(ctx)
	require.NoError(t, err)
	assert.NotNil(t, res2)
}

func TestReportConsistency(t *testing.T) {
	svc := New()
	ctx := context.Background()

	// Call Report multiple times to ensure consistency
	res1, err1 := svc.Report(ctx)
	require.NoError(t, err1)

	res2, err2 := svc.Report(ctx)
	require.NoError(t, err2)

	// Architecture and CPU count should remain the same
	assert.Equal(t, res1.Arch, res2.Arch, "Architecture should be consistent across calls")
	assert.Equal(t, res1.CPU, res2.CPU, "CPU count should be consistent across calls")

	// RAMTotal should remain the same (system total RAM doesn't change)
	assert.Equal(t, res1.RAMTotal, res2.RAMTotal, "RAMTotal should be consistent across calls")

	// RAMAvailable might differ slightly due to system activity, but should be in a reasonable range
	// Allow up to 100MB difference to account for system activity
	diff := res1.RAMAvailable - res2.RAMAvailable
	assert.Less(t, abs(diff), int64(100), "RAMAvailable should be relatively consistent (within 100MB)")
}

func BenchmarkReport(b *testing.B) {
	svc := New()
	ctx := context.Background()

	b.ResetTimer()
	for b.Loop() {
		_, err := svc.Report(ctx)
		if err != nil {
			b.Fatalf("Report failed: %v", err)
		}
	}
}
