package core

import (
	"database/sql/driver"
	"encoding/json"
	"fmt"
	"strings"
)

// Arch represents a supported CPU architecture.
type Arch uint8

// Arch enum values.
const (
	ArchUnknown Arch = iota
	ArchAmd64
	ArchArm64
)

// String returns the string representation of the architecture.
func (a Arch) String() string {
	switch a {
	case ArchAmd64:
		return "amd64"
	case ArchArm64:
		return "arm64"
	default:
		return "unknown"
	}
}

// Scan implements the sql.Scanner interface for reading Arch from the database.
func (a *Arch) Scan(v interface{}) error {
	var s string
	switch t := v.(type) {
	case string:
		s = t
	case []byte:
		s = string(t)
	default:
		return fmt.Errorf("unable to scan CPU architecture (%q) of type %T", t, t)
	}

	return a.Set(s)
}

// Set implements the flag.Value interface.
func (a *Arch) Set(s string) error {
	switch {
	case strings.EqualFold(s, "amd64"):
		*a = ArchAmd64
	case strings.EqualFold(s, "arm64"):
		*a = ArchArm64
	default:
		*a = ArchUnknown
	}

	return nil
}

// Value implements the driver.Valuer interface for writing Arch to the database.
func (a Arch) Value() (driver.Value, error) {
	return a.String(), nil
}

// MarshalJSON implements the json.Marshaler interface for serializing Arch to JSON.
func (a Arch) MarshalJSON() ([]byte, error) {
	return json.Marshal(a.String())
}

// UnmarshalJSON implements the json.Unmarshaler interface for deserializing Arch from JSON.
func (a *Arch) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err != nil {
		return err
	}

	return a.Set(s)
}
