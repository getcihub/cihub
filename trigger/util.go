package trigger

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/getcihub/cihub/core"
)

// extractLabel searches for a cihub-prefixed label in the provided label slice.
//
// It returns the first label that starts with the cihub prefix and true if found,
// or an empty string and false if no matching label exists.
//
// The function performs a linear search through the labels, returning immediately
// when a match is found.
func extractLabel(labels []string) (string, bool) {
	for _, label := range labels {
		if strings.HasPrefix(label, labelPrefix) {
			return label, true
		}
	}

	return "", false
}

// extractResource parses a cihub-formatted label into a Resource specification.
//
// The label must follow the pattern: cihub-<cpu>cpu-<ram>(mb|gb)[-arch]
// Examples:
//   - "cihub-2cpu-4gb" -> 2 CPU, 4096 MB RAM, amd64 architecture
//   - "cihub-4cpu-8gb-arm64" -> 4 CPU, 8192 MB RAM, arm64 architecture
//   - "cihub-1cpu-512mb" -> 1 CPU, 512 MB RAM, amd64 architecture
//
// Returns an error if the label format is invalid or if CPU/RAM values cannot be parsed.
func extractResource(label string) (*core.Resource, error) {
	matches := regexp.MustCompile(labelPattern).FindStringSubmatch(label)
	if matches == nil {
		return nil, fmt.Errorf("invalid label format: %s (expected: cihub-<cpu>cpu-<ram>(mb|gb)[-arch])", label)
	}

	cpu, err := strconv.ParseInt(matches[1], 10, 64)
	if err != nil {
		return nil, fmt.Errorf("invalid CPU specification in label %s: %w", label, err)
	}

	ram, err := parseRAM(matches[2], matches[3])
	if err != nil {
		return nil, fmt.Errorf("invalid RAM specification in label %s: %w", label, err)
	}

	arch, err := parseArch(matches[4])
	if err != nil {
		return nil, fmt.Errorf("invalid architecture in label %s: %w", label, err)
	}

	return &core.Resource{
		Arch:     arch,
		CPU:      cpu,
		RAMTotal: ram,
	}, nil
}

// parseArch converts an architecture string to a core.Arch value.
// Returns amd64 as the default if the input is empty.
func parseArch(s string) (core.Arch, error) {
	if s == "" {
		return core.ArchUnknown, nil
	}

	var arch core.Arch
	if err := arch.Set(s); err != nil {
		return core.ArchUnknown, err
	}

	return arch, nil
}

// parseRAM converts a RAM value and unit (mb/gb) to megabytes.
func parseRAM(value, unit string) (int64, error) {
	ram, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return 0, err
	}

	if strings.EqualFold(unit, "gb") {
		ram *= 1024
	}

	return ram, nil
}
