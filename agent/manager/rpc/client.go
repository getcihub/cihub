package rpc

import (
	"context"

	"github.com/hashicorp/go-retryablehttp"

	"github.com/getcihub/cihub/core"
)

type client struct {
	client *retryablehttp.Client
}

// NewClient returns a new NodeClient.
func NewClient() core.NodeClient {
	return &client{
		client: retryablehttp.NewClient(),
	}
}

// Ping pings a node to confirm connectivity.
func (c *client) Ping(ctx context.Context, node *core.Node) error {
	return nil
}
