// Package label provides dynamic label parsing for GitHub Actions runner specifications.
package label

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/getcihub/cihub/core"
)

const labelPrefix = "cihub-"

// Label represents resource specifications for runners parsed from a label string.
// Labels are requested by GitHub Actions jobs via runs-on field.
//
// Format for dynamic labels: cihub-<cpu>cpu-<ram>(mb|gb)[-<arch>]
// Examples:
//   - cihub-2cpu-4gb        → 2 CPU, 4096 MB RAM, amd64
//   - cihub-4cpu-8gb-arm64  → 4 CPU, 8192 MB RAM, arm64
//   - cihub-2cpu-2048mb     → 2 CPU, 2048 MB RAM, amd64
type Label struct {
	Arch core.Arch `json:"arch"` // CPU architecture (amd64, arm64), defaults to amd64
	RAM  int64     `json:"ram"`  // RAM in megabytes
	CPU  int64     `json:"cpu"`  // CPU cores allocated
}

// Parse parses a label string and returns a Label struct.
// Format: cihub-<cpu>cpu-<ram>(mb|gb)[-<arch>]
// Examples:
//   - cihub-2cpu-4gb
//   - cihub-4cpu-8gb-arm64
//   - cihub-2cpu-2048mb-amd64
//
// Returns nil if the label doesn't match the cihub- pattern.
// Returns error if the label matches the pattern but is invalid.
func Parse(s string) (*Label, error) {
	if !strings.HasPrefix(s, labelPrefix) {
		return nil, nil
	}

	// Pattern: cihub-<digits>cpu-<digits>(mb|gb)[-arm64|-amd64]
	pattern := `^cihub-(\d+)cpu-(\d+)(mb|gb)(?:-(amd64|arm64))?$`
	regex := regexp.MustCompile(pattern)

	matches := regex.FindStringSubmatch(s)
	if matches == nil {
		return nil, fmt.Errorf("invalid cihub label format: %s (expected: cihub-<cpu>cpu-<ram>(mb|gb)[-arch])", s)
	}

	// Parse CPU
	cpu, err := strconv.ParseInt(matches[1], 10, 64)
	if err != nil {
		return nil, fmt.Errorf("invalid CPU specification in label %s: %w", s, err)
	}

	// Parse RAM
	ram, err := strconv.ParseInt(matches[2], 10, 64)
	if err != nil {
		return nil, fmt.Errorf("invalid RAM specification in label %s: %w", s, err)
	}

	// Convert to megabytes if needed
	unit := matches[3] // "mb" or "gb"
	if unit == "gb" {
		ram = ram * 1024
	}

	// Parse architecture (defaults to amd64)
	arch := core.ArchAmd64
	if matches[4] != "" {
		if err := arch.Set(matches[4]); err != nil {
			return nil, err
		}
	}

	return &Label{
		CPU:  cpu,
		RAM:  ram,
		Arch: arch,
	}, nil
}

// Validate validates the label configuration.
// Returns error if CPU <= 0, RAM <= 0, or Arch is not amd64 or arm64.
func (l *Label) Validate() error {
	switch {
	case l.CPU <= 0:
		return fmt.Errorf("invalid CPU specification: %d", l.CPU)
	case l.RAM <= 0:
		return fmt.Errorf("invalid RAM specification: %d", l.RAM)
	case l.Arch != core.ArchAmd64 && l.Arch != core.ArchArm64:
		return fmt.Errorf("invalid architecture: %s (must be amd64 or arm64)", l.Arch)
	default:
		return nil
	}
}

// Resolve finds the first cihub- prefixed label from the job labels and parses it.
// Returns nil if no cihub- label is found or if parsing fails.
// If parsing fails for a cihub- label, returns an error.
func Resolve(labels []string) (*Label, error) {
	for _, label := range labels {
		l, err := Parse(label)
		if err != nil {
			return nil, err
		}
		if l != nil {
			return l, nil
		}
	}
	return nil, nil
}

// Has returns true if any of the provided labels start with "cihub-".
// Used to check if a job's requested labels include cihub labels.
func Has(labels []string) bool {
	for _, label := range labels {
		if strings.HasPrefix(label, labelPrefix) {
			return true
		}
	}
	return false
}
