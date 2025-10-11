package core

import "context"

// SnapshotMount contains mount point information for a snapshot.
type SnapshotMount struct {
	// Source is the path to mount source
	Source string
	// Type is the filesystem type
	Type string
}

// SnapshotService manages filesystem snapshots for runner VMs.
type SnapshotService interface {
	// Create creates a new snapshot from an image for a runner.
	Create(ctx context.Context, name string, ref string) (*SnapshotMount, error)

	// Delete removes a snapshot and release its resources.
	Delete(ctx context.Context, name string) error

	// Exists checks if a snapshot exists.
	Exists(ctx context.Context, name string) (bool, error)

	// Find returns mount information for an existing snaphot.
	Find(ctx context.Context, name string) (*SnapshotMount, error)
}
