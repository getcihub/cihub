package core

import "context"

type (
	// Resource represents the resources of a machine.
	Resource struct {
		Arch         Arch  `json:"arch"`
		CPU          int64 `json:"cpu"`
		RAMTotal     int64 `json:"ram_total"`
		RAMAvailable int64 `json:"ram_available"`
	}

	// ResourceLimit represents limits on machine resource utilisation.
	ResourceLimit struct {
		CPU int64 `json:"cpu"`
		RAM int64 `json:"ram"`
	}

	// ResourceService provides access to machine resources.
	ResourceService interface {
		// Report reports resource of the machine
		Report(ctx context.Context) (*Resource, error)
	}
)
