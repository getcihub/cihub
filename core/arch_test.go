package core

import (
	"encoding/json"
	"testing"
)

func TestArchMarshalJSON(t *testing.T) {
	tests := []struct {
		name string
		arch Arch
		want string
	}{
		{
			name: "amd64",
			arch: ArchAmd64,
			want: `"amd64"`,
		},
		{
			name: "arm64",
			arch: ArchArm64,
			want: `"arm64"`,
		},
		{
			name: "unknown",
			arch: ArchUnknown,
			want: `"unknown"`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := tt.arch.MarshalJSON()
			if err != nil {
				t.Fatalf("MarshalJSON() error = %v, want nil", err)
			}
			if string(got) != tt.want {
				t.Errorf("MarshalJSON() = %q, want %q", string(got), tt.want)
			}
		})
	}
}

func TestArchUnmarshalJSON(t *testing.T) {
	tests := []struct {
		name    string
		data    string
		want    Arch
		wantErr bool
	}{
		{
			name:    "amd64",
			data:    `"amd64"`,
			want:    ArchAmd64,
			wantErr: false,
		},
		{
			name:    "arm64",
			data:    `"arm64"`,
			want:    ArchArm64,
			wantErr: false,
		},
		{
			name:    "AMD64 uppercase",
			data:    `"AMD64"`,
			want:    ArchAmd64,
			wantErr: false,
		},
		{
			name:    "ARM64 uppercase",
			data:    `"ARM64"`,
			want:    ArchArm64,
			wantErr: false,
		},
		{
			name:    "unknown string",
			data:    `"unknown"`,
			want:    ArchUnknown,
			wantErr: false,
		},
		{
			name:    "invalid string",
			data:    `"invalid"`,
			want:    ArchUnknown,
			wantErr: false,
		},
		{
			name:    "invalid json",
			data:    `invalid`,
			want:    ArchUnknown,
			wantErr: true,
		},
		{
			name:    "number instead of string",
			data:    `123`,
			want:    ArchUnknown,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var got Arch
			err := got.UnmarshalJSON([]byte(tt.data))
			if (err != nil) != tt.wantErr {
				t.Errorf("UnmarshalJSON() error = %v, wantErr %v", err, tt.wantErr)
			}
			if got != tt.want {
				t.Errorf("UnmarshalJSON() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestArchJSONRoundtrip(t *testing.T) {
	tests := []struct {
		name string
		arch Arch
	}{
		{
			name: "amd64",
			arch: ArchAmd64,
		},
		{
			name: "arm64",
			arch: ArchArm64,
		},
		{
			name: "unknown",
			arch: ArchUnknown,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Marshal to JSON
			data, err := json.Marshal(tt.arch)
			if err != nil {
				t.Fatalf("json.Marshal() error = %v", err)
			}

			// Unmarshal back
			var got Arch
			if err := json.Unmarshal(data, &got); err != nil {
				t.Fatalf("json.Unmarshal() error = %v", err)
			}

			if got != tt.arch {
				t.Errorf("roundtrip failed: got %v, want %v", got, tt.arch)
			}
		})
	}
}

func TestResourceMarshalJSON(t *testing.T) {
	resource := &Resource{
		Arch:         ArchAmd64,
		CPU:          4,
		RAMTotal:     8192,
		RAMAvailable: 4096,
	}

	data, err := json.Marshal(resource)
	if err != nil {
		t.Fatalf("json.Marshal() error = %v", err)
	}

	// Verify arch is serialized as string
	var m map[string]interface{}
	if err := json.Unmarshal(data, &m); err != nil {
		t.Fatalf("json.Unmarshal() error = %v", err)
	}

	if arch, ok := m["arch"].(string); !ok || arch != "amd64" {
		t.Errorf("arch field = %v, want \"amd64\"", m["arch"])
	}
}

func TestResourceUnmarshalJSON(t *testing.T) {
	jsonData := `{
		"arch": "arm64",
		"cpu": 8,
		"ram_total": 16384,
		"ram_available": 8192
	}`

	var resource Resource
	if err := json.Unmarshal([]byte(jsonData), &resource); err != nil {
		t.Fatalf("json.Unmarshal() error = %v", err)
	}

	if resource.Arch != ArchArm64 {
		t.Errorf("Arch = %v, want ArchArm64", resource.Arch)
	}
	if resource.CPU != 8 {
		t.Errorf("CPU = %d, want 8", resource.CPU)
	}
	if resource.RAMTotal != 16384 {
		t.Errorf("RAMTotal = %d, want 16384", resource.RAMTotal)
	}
	if resource.RAMAvailable != 8192 {
		t.Errorf("RAMAvailable = %d, want 8192", resource.RAMAvailable)
	}
}
