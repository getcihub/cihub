package core

import "context"

// ImageService manages container images used for runner VMs.
type ImageService interface {
	// Pull downloads a container image from a registry.
	Pull(ctx context.Context, ref string) error

	// Exists checks if an image is available locally.
	Exists(ctx context.Context, ref string) (bool, error)

	// Delete removes an image from local storage.
	Delete(ctx context.Context, ref string) error
}
