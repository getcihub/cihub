package session

import "time"

// Config provides the session configuration.
type Config struct {
	Secure  bool
	Secret  string
	Timeout time.Duration
}

// NewConfig returns a new session configuration.
func NewConfig(secret string, timeout time.Duration, secure bool) Config {
	return Config{
		Secure:  secure,
		Secret:  secret,
		Timeout: timeout,
	}
}
