package refresher

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/getcihub/cihub/core"
)

// expiryDelta determines how earlier a token should be considered
// expired than its actual expiration time. It is used to avoid late
// expirations due to client-server time mismatches.
const expiryDelta = time.Minute

type refresher struct {
	client       *http.Client
	clientID     string
	clientSecret string
	endpoint     string
	users        core.UserStore
}

// New returns a new Refresher.
func New(store core.UserStore, client *http.Client, config Config) core.Refresher {
	return &refresher{
		client:       client,
		clientID:     config.ClientID,
		clientSecret: config.ClientSecret,
		endpoint:     config.Endpoint,
		users:        store,
	}
}

func (r *refresher) Refresh(ctx context.Context, user *core.User, force bool) error {
	if !expired(user) && !force {
		return nil
	}

	values := url.Values{}
	values.Set("client_id", r.clientID)
	values.Set("client_secret", r.clientSecret)
	values.Set("grant_type", "refresh_token")
	values.Set("refresh_token", user.Refresh)

	reqBody := strings.NewReader(values.Encode())
	req, err := http.NewRequest("POST", r.endpoint, reqBody)
	if err != nil {
		return err
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")

	res, err := r.client.Do(req)
	if err != nil {
		return err
	}
	defer res.Body.Close()

	// Read the response body
	body, err := io.ReadAll(res.Body)
	if err != nil {
		return err
	}

	if res.StatusCode > 299 {
		out := new(tokenError)
		err = json.Unmarshal(body, out)
		if err != nil {
			return err
		}
		return out
	}

	out := new(tokenGrant)
	err = json.Unmarshal(body, out)
	if err != nil {
		return err
	}

	user.Access = out.Access
	user.Refresh = out.Refresh
	user.Expiry = time.Now().
		Add(time.Duration(out.Expires) * time.Second).Unix()

	return r.users.Update(ctx, user)
}

// expired reports whether the token is expired.
func expired(user *core.User) bool {
	if len(user.Refresh) == 0 {
		return false
	}
	if user.Expiry == 0 && len(user.Access) != 0 {
		return false
	}
	return time.Unix(user.Expiry, 0).Add(-expiryDelta).
		Before(time.Now())
}

// tokenGrant is the token returned by the token endpoint.
type tokenGrant struct {
	Access  string `json:"access_token"`
	Refresh string `json:"refresh_token"`
	Expires int64  `json:"expires_in"`
}

// tokenError is the error returned when the token endpoint
// returns a non-2XX HTTP status code.
type tokenError struct {
	Code    string `json:"error"`
	Message string `json:"error_description"`
}

func (t *tokenError) Error() string {
	return t.Message
}
