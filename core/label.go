package core

import (
	"fmt"
	"strings"
)

// Label represents resource specifications for runners.
// Labels are requested by GitHub Actions jobs via runs-on field.
// Label ID must start with "cihub-" prefix.
type Label struct {
	ID     string `json:"id" koanf:"id"`     // Unique identifier, must start with "cihub-"
	Arch   string `json:"arch" koanf:"arch"` // CPU architecture (amd64, arm64)
	OS     string `json:"os" koanf:"os"`     // Operating system image (OCI container image reference)
	Memory int64  `json:"memory" koanf:"memory"` // RAM in megabytes
	VCPU   int64  `json:"vcpu" koanf:"vcpu"`     // CPU cores allocated
}

// Validate validates the label configuration.
// Returns error if ID doesn't start with "cihub-", Arch is not amd64 or arm64, VCPU <= 0, or Memory <= 0.
func (label *Label) Validate() error {
	switch {
	case !strings.HasPrefix(label.ID, "cihub-"):
		return fmt.Errorf("invalid label ID prefix: %s", label.ID)
	case label.Arch != "amd64" && label.Arch != "arm64":
		return fmt.Errorf("invalid architecture: %s (must be amd64 or arm64)", label.Arch)
	case label.VCPU <= 0:
		return fmt.Errorf("invalid VCPU specification: %d", label.VCPU)
	case label.Memory <= 0:
		return fmt.Errorf("invalid memory specification: %d", label.Memory)
	default:
		return nil
	}
}

// Labels is a map of available label configurations keyed by label ID.
type Labels map[string]Label

// Has returns true if any of the provided labels exist in the configured Labels.
// Used to check if a job's requested labels are supported.
func (labels Labels) Has(s []string) bool {
	for _, l := range s {
		_, ok := labels[l]
		if ok {
			return true
		}
	}
	return false
}
