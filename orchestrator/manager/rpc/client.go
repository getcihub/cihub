package rpc

import (
	"context"
	"encoding/json"
	"io"
	"strings"
	"time"

	"github.com/hashicorp/go-retryablehttp"
	"github.com/oxtoacart/bpool"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/orchestrator/manager"
	"github.com/getcihub/cihub/store/shared/db"
)

var bufpool = bpool.NewBufferPool(64)

type Client struct {
	client *retryablehttp.Client
	server string
	secret string
}

func NewClient(server, secret string) *Client {
	client := retryablehttp.NewClient()
	client.RetryMax = 30
	client.RetryWaitMax = time.Second * 10
	client.RetryWaitMin = time.Second * 1
	client.Logger = nil
	return &Client{
		client: client,
		server: strings.TrimSuffix(server, "/"),
		secret: secret,
	}
}

func (c *Client) Ping(ctx context.Context, machine string) error {
	in := &pingRequest{Machine: machine}
	return c.send(ctx, "/rpc/v1/ping", in, nil)
}

func (c *Client) Request(ctx context.Context, labels []string) (*core.Job, error) {
	timeout, cancel := context.WithTimeout(ctx, time.Minute)
	defer cancel()

	in := &requestRequest{Labels: labels}
	out := &core.Job{}
	err := c.send(timeout, "/rpc/v1/request", in, out)

	// The request is performing long polling and is subject
	// to a client-side and server-side timeout. The timeout
	// error is therefore expected behavior, and is not
	// considered an error by the system.
	if err == context.DeadlineExceeded {
		return nil, nil // no error
	}

	return out, err
}

func (c *Client) Accept(ctx context.Context, jobID int64, machine string) (*core.Job, error) {
	in := &acceptRequest{JobID: jobID, Machine: machine}
	out := &core.Job{}
	err := c.send(ctx, "/rpc/v1/accept", in, out)
	return out, err
}

func (c *Client) Details(ctx context.Context, jobID int64) (*core.RunnerWithToken, error) {
	in := &detailsRequest{Job: jobID}
	out := &core.RunnerWithToken{}
	err := c.send(ctx, "/rpc/v1/details", in, out)
	return out, err
}

func (c *Client) Watch(ctx context.Context, runnerID int64) (bool, error) {
	in := &watchRequest{RunnerID: runnerID}
	out := &watchResponse{}
	err := c.send(ctx, "/rpc/v1/watch", in, out)
	return out.Done, err
}

func (c *Client) send(ctx context.Context, path string, in, out interface{}) error {
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

var _ manager.RunnerManager = (*Client)(nil)
