package db

import (
	"fmt"
	"strings"
)

// Driver defines database driver.
type Driver int

// Database driver enums.
const (
	Unknown Driver = iota
	Sqlite
	Mysql
	Postgres
)

// Set implements the flag.Value interface.
func (d *Driver) Set(s string) error {
	switch {
	case strings.EqualFold(s, "sqlite3"):
		*d = Sqlite
	case strings.EqualFold(s, "mysql"):
		*d = Mysql
	case strings.EqualFold(s, "postgres"):
		*d = Postgres
	default:
		return fmt.Errorf("invalid database driver value '%s'", s)
	}

	return nil
}

// String implements the fmt.Stringer interface.
func (d Driver) String() string {
	switch d {
	case Sqlite:
		return "sqlite3"
	case Mysql:
		return "mysql"
	case Postgres:
		return "postgres"
	default:
		return "unknown"
	}
}

// UnmarshalText implements encoding.TextUnmarshaler.
func (d *Driver) UnmarshalText(text []byte) error {
	return d.Set(string(text))
}
