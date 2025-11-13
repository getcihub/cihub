package client

import (
	"context"
	"encoding/json"
	"io"
	"strings"
	"time"

	"github.com/hashicorp/go-retryablehttp"
	"github.com/oxtoacart/bpool"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

var bufpool = bpool.NewBufferPool(64)

type client struct {
	client *retryablehttp.Client
	server string
	secret string
}

func NewClient(server, secret string) core.Client {
	c := retryablehttp.NewClient()
	c.RetryMax = 30
	c.RetryWaitMax = time.Second * 10
	c.RetryWaitMin = time.Second * 1
	c.Logger = nil

	return &client{
		client: c,
		server: strings.TrimSuffix(server, "/"),
		secret: secret,
	}
}

func (c *client) Join(ctx context.Context) error {
	return c.send(ctx, "/rpc/v1/join", nil, nil)
}

func (c *client) Leave(ctx context.Context) error {
	return c.send(ctx, "/rpc/v1/leave", nil, nil)
}

func (c *client) Ping(ctx context.Context, resource *core.Resource) error {
	return c.send(ctx, "/rpc/v1/ping", resource, nil)
}

func (c *client) Request(ctx context.Context) (*core.Runner, error) {
	timeout, cancel := context.WithTimeout(ctx, time.Second*5)
	defer cancel()

	out := &core.Runner{}
	err := c.send(timeout, "/rpc/v1/request", nil, out)

	// The request is performing long polling and is subject
	// to a client-side and server-side timeout. The timeout
	// error is therefore expected behavior, and is not
	// considered an error by the system.
	if err == context.DeadlineExceeded {
		return nil, nil // no error
	}

	return out, err
}

func (c *client) Accept(ctx context.Context, runner *core.Runner) error {
	return c.send(ctx, "/rpc/v1/accept", runner, nil)
}

func (c *client) Register(ctx context.Context, runner *core.Runner) (*core.RunnerWithToken, error) {
	out := &core.RunnerWithToken{}
	err := c.send(ctx, "/rpc/v1/register", runner, out)
	return out, err
}

func (c *client) Lock(ctx context.Context, runner *core.Runner) error {
	return c.send(ctx, "/rpc/v1/lock", runner, nil)
}

func (c *client) Unlock(ctx context.Context, runner *core.Runner) error {
	return c.send(ctx, "/rpc/v1/unlock", runner, nil)
}

func (c *client) Watch(ctx context.Context, runner *core.Runner) (bool, error) {
	err := c.send(ctx, "/rpc/v1/watch", runner, nil)
	if err != nil {
		return true, nil
	}
	return false, err
}

func (c *client) send(ctx context.Context, path string, in, out interface{}) error {
	buf := bufpool.Get()
	defer bufpool.Put(buf)

	err := json.NewEncoder(buf).Encode(in)
	if err != nil {
		return err
	}

	url := c.server + path
	req, err := retryablehttp.NewRequest("POST", url, buf)
	if err != nil {
		return err
	}
	req = req.WithContext(ctx)
	req.Header.Set("X-CIHub-Token", c.secret)

	res, err := c.client.Do(req)
	if res != nil {
		defer res.Body.Close()
	}

	if err != nil {
		return err
	}

	// Check the response for a 409 conflict. This indicates an
	// optimistic lock error, in which case multiple clients may
	// be attempting to update the same record. Convert this error
	// code to a proper error.
	if res.StatusCode == 409 {
		return db.ErrOptimisticLock
	}

	// Check the response for a 524 deadline exceeded. This is a
	// custom status code that indicates the server canceled the
	// request due to an internal polling timeout (this is normal).
	if res.StatusCode == 524 {
		return context.DeadlineExceeded
	}

	// Check the response for a 401 unauthorized. This indicates
	// that the machine token does not exist or is no longer valid
	if res.StatusCode == 401 {
		return ErrMachineNotFound
	}

	if res.StatusCode > 299 {
		body, _ := io.ReadAll(res.Body)
		return &serverError{
			Status:  res.StatusCode,
			Message: string(body),
		}
	}

	// Check the response for a 204 no content. This indicates
	// the response body is empty and should be discarded.
	if res.StatusCode == 204 || out == nil {
		return nil
	}

	return json.NewDecoder(res.Body).Decode(out)
}
