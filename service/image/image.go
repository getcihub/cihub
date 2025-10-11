package image

import (
	"context"
	"fmt"

	"github.com/containerd/containerd"
	"github.com/containerd/containerd/errdefs"
	"github.com/containerd/nerdctl/pkg/imgutil/dockerconfigresolver"
	"github.com/distribution/reference"

	"github.com/getcihub/cihub/core"
)

type service struct {
	client      *containerd.Client
	snapshotter string
}

func New(client *containerd.Client, snapshotter string) core.ImageService {
	return &service{
		client:      client,
		snapshotter: snapshotter,
	}
}

func (s *service) Pull(ctx context.Context, ref string) error {
	parsed, err := reference.ParseDockerRef(ref)
	if err != nil {
		return fmt.Errorf("image: failed to parse docker image reference, err: %w", err)
	}

	resolver, err := dockerconfigresolver.New(ctx, reference.Domain(parsed))
	if err != nil {
		return fmt.Errorf("image: failed to get docker config resolver, err: %w", err)
	}

	pullOpts := []containerd.RemoteOpt{
		containerd.WithPullUnpack,
		containerd.WithResolver(resolver),
		containerd.WithPullSnapshotter(s.snapshotter),
	}

	_, err = s.client.Pull(ctx, ref, pullOpts...)
	if err != nil {
		return fmt.Errorf("image: failed to pull image, err: %w", err)
	}

	return nil
}

func (s *service) Exists(ctx context.Context, ref string) (bool, error) {
	_, err := s.client.GetImage(ctx, ref)
	if err != nil {
		if errdefs.IsNotFound(err) {
			return false, nil
		} else {
			return false, fmt.Errorf("image: failed to check image existence, err: %w", err)
		}
	}

	return true, nil
}

func (s *service) Delete(ctx context.Context, ref string) error {
	image, err := s.client.GetImage(ctx, ref)
	if err != nil {
		// image does not exists?
		if errdefs.IsNotFound(err) {
			return nil
		} else {
			return fmt.Errorf("agent: failed to get image to delete, err: %w", err)
		}
	}

	err = s.client.ImageService().Delete(ctx, image.Name())
	if err != nil {
		// already deleted?
		if errdefs.IsNotFound(err) {
			return nil
		} else {
			return fmt.Errorf("agent: failed to delete, err: %w", err)
		}
	}

	return nil
}
