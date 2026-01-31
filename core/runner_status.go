package core

import (
	"database/sql/driver"
	"encoding/json"
	"fmt"
	"strings"
)

// RunnerStatus specifies the status of a runner.
type RunnerStatus uint8

const (
	// RunnerStatusUnknown is an unknown status.
	RunnerStatusUnknown RunnerStatus = iota + 1
	// RunnerStatusPending indicates a Runner is pending creation.
	RunnerStatusPending
	// RunnerStatusRegistered indicates a Runner is registered to GitHub.
	RunnerStatusRegistered
	// RunnerStatusIdle indicates a Runner is online and not running any job.
	RunnerStatusIdle
	// RunnerStatusBusy indicates a Runner is running a job.
	RunnerStatusBusy
	// RunnerStatusCompleted indicates a runner has completed running a job.
	RunnerStatusCompleted
)

// MarshalJSON implements the json.Marshaler interface.
func (r RunnerStatus) MarshalJSON() ([]byte, error) {
	return json.Marshal(r.String())
}

// UnmarshalJSON implements the json.Unmarshaler interface.
func (r *RunnerStatus) UnmarshalJSON(b []byte) error {
	var s string
	if err := json.Unmarshal(b, &s); err != nil {
		return err
	}

	return r.Set(s)
}

// Set implements the flag.Value interface.
func (r *RunnerStatus) Set(s string) error {
	switch {
	case strings.EqualFold(s, "pending"):
		*r = RunnerStatusPending
	case strings.EqualFold(s, "registered"):
		*r = RunnerStatusRegistered
	case strings.EqualFold(s, "idle"):
		*r = RunnerStatusIdle
	case strings.EqualFold(s, "busy"):
		*r = RunnerStatusBusy
	case strings.EqualFold(s, "completed"):
		*r = RunnerStatusCompleted
	default:
		return fmt.Errorf("invalid runner status value '%s'", s)
	}

	return nil
}

// Scan implements the sql.Scanner interface.
func (r *RunnerStatus) Scan(v interface{}) error {
	var s string
	switch t := v.(type) {
	case string:
		s = t
	case []byte:
		s = string(t)
	default:
		return fmt.Errorf("unable to scan runner status (%q) of type %T", t, t)
	}

	return r.Set(s)
}

// String implements the fmt.Stringer interface.
func (r RunnerStatus) String() string {
	switch r {
	case RunnerStatusPending:
		return "pending"
	case RunnerStatusRegistered:
		return "registered"
	case RunnerStatusIdle:
		return "idle"
	case RunnerStatusBusy:
		return "busy"
	case RunnerStatusCompleted:
		return "completed"
	default:
		return "unknown"
	}
}

// Value implement the driver.Valuer interface.
func (r RunnerStatus) Value() (driver.Value, error) {
	return r.String(), nil
}
