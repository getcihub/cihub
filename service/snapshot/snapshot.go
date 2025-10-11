package snapshot

import (
	"context"
	"fmt"

	"github.com/containerd/containerd"
	"github.com/containerd/containerd/errdefs"
	"github.com/containerd/containerd/leases"
	"github.com/opencontainers/image-spec/identity"

	"github.com/getcihub/cihub/core"
)

type service struct {
	client      *containerd.Client
	snapshotter string
	leases      leases.Manager
}

func New(client *containerd.Client, snapshotter string) core.SnapshotService {
	return &service{
		client:      client,
		snapshotter: snapshotter,
		leases:      client.LeasesService(),
	}
}

// Create creates a new snapshot from an image for a runner.
func (s *service) Create(ctx context.Context, name string, ref string) (*core.SnapshotMount, error) {
	// get lease or create it
	lease, err := s.getOrCreateLease(ctx, name)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to acquire lease, err: %w", err)
	}

	ctx = leases.WithLease(ctx, lease.ID)

	// get the image, must have been pulled by image service before
	image, err := s.client.GetImage(ctx, ref)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to get image, err: %w", err)
	}

	// ensure image is unpacked
	isUnpacked, err := image.IsUnpacked(ctx, s.snapshotter)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to check if image is unpacked, err: %w", err)
	}

	if !isUnpacked {
		if err := image.Unpack(ctx, s.snapshotter); err != nil {
			return nil, fmt.Errorf("snapshot: failed to unpack image, err: %w", err)
		}
	}

	// Get the image rootfs chain ID
	imageContent, err := image.RootFS(ctx)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to get image root FS, err: %w", err)
	}

	// Get snapshot service
	ss := s.client.SnapshotService(s.snapshotter)

	// Prepare the snapshot
	_, err = ss.Prepare(ctx, name, identity.ChainID(imageContent).String())
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to prepare snapshot, err: %w", err)
	}

	// Get mount information
	mounts, err := ss.Mounts(ctx, name)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to get snapshot mounts, err: %w", err)
	}

	if len(mounts) == 0 {
		return nil, fmt.Errorf("snapshot: no mounts available for snapshot")
	}

	return &core.SnapshotMount{
		Source: mounts[0].Source,
		Type:   mounts[0].Type,
	}, nil
}

// Delete removes a snapshot and release its resources.
func (s *service) Delete(ctx context.Context, name string) error {
	// get lease or create it
	lease, err := s.getOrCreateLease(ctx, name)
	if err != nil {
		return fmt.Errorf("snapshot: failed to acquire lease, err: %w", err)
	}

	ctx = leases.WithLease(ctx, lease.ID)

	// remove the snapshot
	ss := s.client.SnapshotService(s.snapshotter)
	if err := ss.Remove(ctx, name); err != nil {
		if !errdefs.IsNotFound(err) {
			return fmt.Errorf("snapshot: failed to remove snapshot, err: %w", err)
		}
	}

	// cancel the lease to release resources
	if err := s.deleteLease(ctx, name); err != nil {
		if !errdefs.IsNotFound(err) {
			return fmt.Errorf("snapshot: failed to cancel lease,, err: %w", err)
		}
	}

	return nil
}

// Exists checks if a snapshot exists.
func (s *service) Exists(ctx context.Context, name string) (bool, error) {
	// get lease or create it
	lease, err := s.getOrCreateLease(ctx, name)
	if err != nil {
		return false, fmt.Errorf("snapshot: failed to acquire lease, err: %w", err)
	}

	ctx = leases.WithLease(ctx, lease.ID)

	ss := s.client.SnapshotService(s.snapshotter)
	_, err = ss.Stat(ctx, name)
	if err != nil {
		if errdefs.IsNotFound(err) {
			return false, nil
		} else {
			return false, fmt.Errorf("snapshot: failed to check snapshot existence, err: %w", err)
		}
	}

	return true, nil
}

// Find returns mount information for an existing snaphot.
func (s *service) Find(ctx context.Context, name string) (*core.SnapshotMount, error) {
	// get lease or create it
	lease, err := s.getOrCreateLease(ctx, name)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to acquire lease, err: %w", err)
	}

	ctx = leases.WithLease(ctx, lease.ID)

	ss := s.client.SnapshotService(s.snapshotter)
	mounts, err := ss.Mounts(ctx, name)
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to get snapshot mounts: %w", err)
	}

	if len(mounts) == 0 {
		return nil, fmt.Errorf("snapshot: no mounts available for snapshot")
	}

	return &core.SnapshotMount{
		Source: mounts[0].Source,
		Type:   mounts[0].Type,
	}, nil
}

func (s *service) getOrCreateLease(ctx context.Context, name string) (*leases.Lease, error) {
	existingLeases, err := s.leases.List(ctx, fmt.Sprintf("id==%s", name))
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to list existing containerd lease, err: %w", err)
	}

	for _, lease := range existingLeases {
		if lease.ID == name {
			return &lease, nil
		}
	}

	lease, err := s.leases.Create(ctx, leases.WithID(name))
	if err != nil {
		return nil, fmt.Errorf("snapshot: failed to create lease %s, err: %w", name, err)
	}

	return &lease, nil
}

func (s *service) deleteLease(ctx context.Context, name string) error {
	lease := leases.Lease{ID: name}

	err := s.leases.Delete(ctx, lease, leases.SynchronousDelete)
	if err != nil {
		return fmt.Errorf("manager: failed to delete lease %s, err: %w", name, err)
	}

	return nil
}
