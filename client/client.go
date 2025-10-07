package client

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/url"
	"strconv"
	"strings"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/hashicorp/go-retryablehttp"
	"github.com/urfave/cli/v3"
)

const (
	pathSelf  = "%s/api/user"
	pathUsers = "%s/api/users"
)

type client struct {
	server string
	token  string
	client *retryablehttp.Client
}

// New returns a new CIHub client
// func New(server, token string) (core.Client, error) {
func New(cmd *cli.Command) (core.Client, error) {
	var (
		server = cmd.String("server")
		token  = cmd.String("auth-token")
	)

	server = strings.TrimPrefix(server, "/")

	if len(server) == 0 {
		return nil, fmt.Errorf("client: server address must be provided")
	}
	if len(token) == 0 {
		return nil, fmt.Errorf("client: access token must be provided")
	}

	httpClient := retryablehttp.NewClient()
	httpClient.RetryMax = 30
	httpClient.RetryWaitMax = time.Second * 10
	httpClient.RetryWaitMin = time.Second * 1
	httpClient.Logger = nil

	c := &client{
		server: strings.TrimRight(server, "/"),
		token:  token,
		client: httpClient,
	}

	return c, nil
}

func (c *client) Self(ctx context.Context) (*core.User, error) {
	out := new(core.User)
	uri := fmt.Sprintf(pathSelf, c.server)
	err := c.get(ctx, uri, out)
	return out, err
}

func (c *client) UserCreate(ctx context.Context, in *core.User) (*core.User, error) {
	out := new(core.User)
	uri := fmt.Sprintf(pathUsers, c.server)
	err := c.post(ctx, uri, in, out)
	return out, err
}

func (c *client) get(ctx context.Context, url string, out interface{}) error {
	return c.do(ctx, url, "GET", nil, out)
}

func (c *client) post(ctx context.Context, url string, in, out interface{}) error {
	return c.do(ctx, url, "POST", in, out)
}

// Response represents the API response wrapper
type Response struct {
	Error  bool            `json:"error"`
	Reason string          `json:"reason"`
	Data   json.RawMessage `json:"data,omitempty"`
}

func (c *client) do(ctx context.Context, path, method string, in, out interface{}) error {
	// Parse and validate URL
	uri, err := url.Parse(path)
	if err != nil {
		return fmt.Errorf("client: invalid URL, err: %w", err)
	}

	// Create HTTP request
	req, err := retryablehttp.NewRequestWithContext(ctx, method, uri.String(), nil)
	if err != nil {
		return fmt.Errorf("client: failed to create HTTP request, err: %w", err)
	}

	// Create request body if input is provided
	if in != nil {
		encoded, err := json.Marshal(in)
		if err != nil {
			return fmt.Errorf("client: failed to marshal request body, err: %w", err)
		}

		buffer := bytes.NewBuffer(encoded)
		req.Body = io.NopCloser(buffer)
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Content-Length", strconv.Itoa(len(encoded)))
	}

	// Add authentication
	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", c.token))

	// Execute request
	res, err := c.client.Do(req)
	if err != nil {
		return fmt.Errorf("client: request failed, err: %w", err)
	}

	defer func() {
		if err := res.Body.Close(); err != nil {
			logger.FromContext(ctx).
				WithError(err).
				Warn("client: failed to close response body")
		}
	}()

	// Decode response wrapper
	var resp Response
	if err := json.NewDecoder(res.Body).Decode(&resp); err != nil {
		return fmt.Errorf("client: failed to decode response, err: %w", err)
	}

	// Handle error responses
	if resp.Error {
		return fmt.Errorf("client: API error (reason: %s)", resp.Reason)
	}

	// Handle non-success status codes
	if res.StatusCode > 299 {
		return fmt.Errorf("client: HTTP %d (reason: %s)", res.StatusCode, resp.Reason)
	}

	// Decode data field into output
	if out != nil && len(resp.Data) > 0 {
		if err := json.Unmarshal(resp.Data, out); err != nil {
			return fmt.Errorf("client: failed to decode response data, err: %w", err)
		}
	}

	return nil
}
