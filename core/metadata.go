package core

import "context"

type (
	Metadata struct {
		RunnerID        string `json:"runner_id"`
		RunnerHostname  string `json:"runner_hostname"`
		RunnerJITConfig string `json:"runner_jit_config"`
	}

	MetadataService interface {
		// Find returns the metadata for a given path.
		Find(ctx context.Context, path string) (*Metadata, error)
	}
)
