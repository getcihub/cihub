package db

import (
	"testing"
)

func TestDriver_String(t *testing.T) {
	tests := []struct {
		name   string
		driver Driver
		want   string
	}{
		{
			name:   "sqlite",
			driver: Sqlite,
			want:   "sqlite3",
		},
		{
			name:   "mysql",
			driver: Mysql,
			want:   "mysql",
		},
		{
			name:   "postgres",
			driver: Postgres,
			want:   "postgres",
		},
		{
			name:   "unknown",
			driver: Unknown,
			want:   "unknown",
		},
		{
			name:   "invalid value",
			driver: Driver(999),
			want:   "unknown",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := tt.driver.String()
			if got != tt.want {
				t.Errorf("Driver.String() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestDriver_Set(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		want    Driver
		wantErr bool
	}{
		{
			name:    "sqlite3 lowercase",
			input:   "sqlite3",
			want:    Sqlite,
			wantErr: false,
		},
		{
			name:    "sqlite3 uppercase",
			input:   "SQLITE3",
			want:    Sqlite,
			wantErr: false,
		},
		{
			name:    "sqlite3 mixed case",
			input:   "SqLiTe3",
			want:    Sqlite,
			wantErr: false,
		},
		{
			name:    "mysql lowercase",
			input:   "mysql",
			want:    Mysql,
			wantErr: false,
		},
		{
			name:    "mysql uppercase",
			input:   "MYSQL",
			want:    Mysql,
			wantErr: false,
		},
		{
			name:    "mysql mixed case",
			input:   "MySql",
			want:    Mysql,
			wantErr: false,
		},
		{
			name:    "postgres lowercase",
			input:   "postgres",
			want:    Postgres,
			wantErr: false,
		},
		{
			name:    "postgres uppercase",
			input:   "POSTGRES",
			want:    Postgres,
			wantErr: false,
		},
		{
			name:    "postgres mixed case",
			input:   "PoStGrEs",
			want:    Postgres,
			wantErr: false,
		},
		{
			name:    "invalid driver",
			input:   "oracle",
			want:    Unknown,
			wantErr: true,
		},
		{
			name:    "empty string",
			input:   "",
			want:    Unknown,
			wantErr: true,
		},
		{
			name:    "invalid with whitespace",
			input:   " sqlite3 ",
			want:    Unknown,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var d Driver
			err := d.Set(tt.input)

			if tt.wantErr {
				if err == nil {
					t.Errorf("Driver.Set(%q) expected error, got nil", tt.input)
				}
			} else {
				if err != nil {
					t.Errorf("Driver.Set(%q) unexpected error: %v", tt.input, err)
				}
			}

			if d != tt.want {
				t.Errorf("Driver.Set(%q) = %v, want %v", tt.input, d, tt.want)
			}
		})
	}
}

func TestDriver_UnmarshalText(t *testing.T) {
	tests := []struct {
		name    string
		input   []byte
		want    Driver
		wantErr bool
	}{
		{
			name:    "sqlite3",
			input:   []byte("sqlite3"),
			want:    Sqlite,
			wantErr: false,
		},
		{
			name:    "mysql",
			input:   []byte("mysql"),
			want:    Mysql,
			wantErr: false,
		},
		{
			name:    "postgres",
			input:   []byte("postgres"),
			want:    Postgres,
			wantErr: false,
		},
		{
			name:    "uppercase sqlite3",
			input:   []byte("SQLITE3"),
			want:    Sqlite,
			wantErr: false,
		},
		{
			name:    "invalid driver",
			input:   []byte("mongodb"),
			want:    Unknown,
			wantErr: true,
		},
		{
			name:    "empty bytes",
			input:   []byte(""),
			want:    Unknown,
			wantErr: true,
		},
		{
			name:    "nil bytes",
			input:   nil,
			want:    Unknown,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var d Driver
			err := d.UnmarshalText(tt.input)

			if tt.wantErr {
				if err == nil {
					t.Errorf("Driver.UnmarshalText(%q) expected error, got nil", tt.input)
				}
			} else {
				if err != nil {
					t.Errorf("Driver.UnmarshalText(%q) unexpected error: %v", tt.input, err)
				}
			}

			if d != tt.want {
				t.Errorf("Driver.UnmarshalText(%q) = %v, want %v", tt.input, d, tt.want)
			}
		})
	}
}

func TestDriver_Enums(t *testing.T) {
	// Verify enum values are distinct
	drivers := []Driver{Unknown, Sqlite, Mysql, Postgres}
	seen := make(map[Driver]bool)

	for _, d := range drivers {
		if seen[d] {
			t.Errorf("duplicate driver value: %v", d)
		}
		seen[d] = true
	}

	// Verify zero value is Unknown
	var d Driver
	if d != Unknown {
		t.Errorf("zero value of Driver = %v, want Unknown (%v)", d, Unknown)
	}
}
